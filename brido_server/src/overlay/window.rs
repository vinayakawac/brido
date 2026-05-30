//! Overlay window — floating egui panel for stealth AI assistance.

use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, Layout, Margin, RichText, ScrollArea,
    Stroke, StrokeKind, Vec2,
};
use std::sync::mpsc;

use super::hotkey::OverlayEvent;

// ── Colours (matching Brido server palette) ─────────────────────────────────
const BG: Color32 = Color32::from_rgb(18, 18, 18);
const SURFACE: Color32 = Color32::from_rgb(30, 30, 30);
const SURFACE_HOVER: Color32 = Color32::from_rgb(42, 42, 42);
const ACCENT: Color32 = Color32::from_rgb(0, 230, 118);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 240, 240);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 160, 160);
const TEXT_DIM: Color32 = Color32::from_rgb(100, 100, 100);
const RED: Color32 = Color32::from_rgb(239, 83, 80);
const YELLOW: Color32 = Color32::from_rgb(255, 202, 40);
const CODE_BG: Color32 = Color32::from_rgb(40, 40, 40);

/// Messages sent from async analysis tasks back to the UI.
pub enum AnalysisResult {
    /// Analysis completed successfully.
    Done {
        response: String,
        model_used: String,
    },
    /// Analysis failed with an error message.
    Error(String),
}

pub struct OverlayApp {
    /// Sender for hotkey events, to pass to the new hotkey listener when restarting.
    hotkey_tx: mpsc::Sender<OverlayEvent>,
    /// Receiver for hotkey events from the background listener thread.
    hotkey_rx: mpsc::Receiver<OverlayEvent>,
    /// Handle to the hotkey listener thread, to stop it when restarting.
    hotkey_handle: Option<super::hotkey::HotkeyHandle>,
    /// Receiver for analysis results from async tasks.
    result_rx: mpsc::Receiver<AnalysisResult>,
    /// Sender for analysis results (cloned into async tasks).
    result_tx: mpsc::Sender<AnalysisResult>,
    /// Handle to the tokio runtime for spawning async analysis tasks.
    rt: tokio::runtime::Handle,
    /// Config for AI provider access.
    config: brido_server::config::Config,
    /// Runtime paths for saving config.
    runtime_env: brido_server::config::RuntimeEnvPaths,

    // ── UI state ────────────────────────────────────────────────────────
    response_text: String,
    model_used: String,
    status_text: String,
    input_text: String,
    is_loading: bool,
    is_visible: bool,
    error_text: Option<String>,
    stealth_applied: bool,
    scroll_to_bottom: bool,

    // ── Settings UI state ───────────────────────────────────────────────
    show_settings: bool,
    settings_openai_key: String,
    settings_anthropic_key: String,
    settings_gemini_key: String,
    settings_openrouter_key: String,
    settings_hotkey_capture: String,
    settings_hotkey_toggle: String,
}

impl OverlayApp {
    pub fn new(
        hotkey_tx: mpsc::Sender<OverlayEvent>,
        hotkey_rx: mpsc::Receiver<OverlayEvent>,
        hotkey_handle: super::hotkey::HotkeyHandle,
        rt: tokio::runtime::Handle,
        config: brido_server::config::Config,
        runtime_env: brido_server::config::RuntimeEnvPaths,
    ) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            hotkey_tx,
            hotkey_rx,
            hotkey_handle: Some(hotkey_handle),
            result_rx,
            result_tx,
            rt,
            settings_openai_key: config.openai_api_key.clone(),
            settings_anthropic_key: config.anthropic_api_key.clone(),
            settings_gemini_key: config.gemini_api_key.clone(),
            settings_openrouter_key: config.openrouter_api_key.clone(),
            settings_hotkey_capture: config.overlay_hotkey_capture.clone(),
            settings_hotkey_toggle: config.overlay_hotkey_toggle.clone(),
            config,
            runtime_env,
            response_text: String::new(),
            model_used: String::new(),
            status_text: "Ready — Ctrl+Shift+Space to capture".to_string(),
            input_text: String::new(),
            is_loading: false,
            is_visible: true,
            error_text: None,
            stealth_applied: false,
            scroll_to_bottom: false,
            show_settings: false,
        }
    }

    /// Trigger a screen capture + AI analysis.
    fn trigger_capture(&mut self, custom_prompt: Option<String>) {
        if self.is_loading {
            return;
        }

        self.is_loading = true;
        self.error_text = None;
        self.status_text = "Capturing screen…".to_string();
        self.response_text.clear();
        self.model_used.clear();

        let config = self.config.clone();
        let tx = self.result_tx.clone();

        self.rt.spawn(async move {
            // Capture runs on a blocking thread because scrap types are !Send
            let capture_result =
                tokio::task::spawn_blocking(super::capture_trigger::capture_screen_base64).await;

            let image_b64 = match capture_result {
                Ok(Ok(b64)) => b64,
                Ok(Err(e)) => {
                    let _ = tx.send(AnalysisResult::Error(format!("Capture failed: {e}")));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(AnalysisResult::Error(format!("Capture task panicked: {e}")));
                    return;
                }
            };

            let result = super::ai_client::analyse(
                &config,
                &image_b64,
                custom_prompt.as_deref(),
            )
            .await;

            match result {
                Ok((response, model_used)) => {
                    let _ = tx.send(AnalysisResult::Done {
                        response,
                        model_used,
                    });
                }
                Err(e) => {
                    let _ = tx.send(AnalysisResult::Error(e));
                }
            }
        });
    }

    /// Submit a manual text question (captures screen as context).
    fn submit_question(&mut self) {
        let question = self.input_text.trim().to_string();
        if question.is_empty() {
            return;
        }
        self.input_text.clear();
        self.trigger_capture(Some(question));
    }

    /// Process pending hotkey events.
    fn poll_hotkeys(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.hotkey_rx.try_recv() {
            match event {
                OverlayEvent::CaptureAndAnalyse => {
                    self.is_visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.trigger_capture(None);
                }
                OverlayEvent::ToggleVisibility => {
                    self.is_visible = !self.is_visible;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.is_visible));
                }
            }
        }
    }

    /// Process pending analysis results.
    fn poll_results(&mut self) {
        while let Ok(result) = self.result_rx.try_recv() {
            self.is_loading = false;
            match result {
                AnalysisResult::Done {
                    response,
                    model_used,
                } => {
                    self.response_text = response;
                    self.model_used = model_used.clone();
                    self.status_text = format!("✓ {model_used}");
                    self.error_text = None;
                    self.scroll_to_bottom = true;
                }
                AnalysisResult::Error(msg) => {
                    self.status_text = "✗ Analysis failed".to_string();
                    self.error_text = Some(msg);
                }
            }
        }
    }

    /// Apply stealth + initial positioning on the first frame.
    fn apply_first_frame_setup(&mut self, ctx: &egui::Context) {
        if self.stealth_applied {
            return;
        }
        self.stealth_applied = true;

        // Position window to right edge of screen.
        let screen = ctx.input(|i| i.screen_rect);
        let panel_width = 380.0;
        let margin = 12.0;
        let x = screen.max.x - panel_width - margin;
        let y = margin;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            egui::pos2(x, y).into(),
        ));
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.heading(RichText::new("Settings").color(TEXT_PRIMARY));
                ui.add_space(8.0);
                
                ui.label(RichText::new("API Keys").color(TEXT_SECONDARY));
                ui.add_space(4.0);

                egui::Grid::new("settings_api_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("OpenAI:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_openai_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("Anthropic:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_anthropic_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("Gemini:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_gemini_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("OpenRouter:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_openrouter_key).password(true));
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.label(RichText::new("Hotkeys (Ctrl+Shift+)").color(TEXT_SECONDARY));
                ui.add_space(4.0);
                
                egui::Grid::new("settings_hotkey_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Capture:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_capture).desired_width(60.0));
                        ui.end_row();

                        ui.label(RichText::new("Toggle:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_toggle).desired_width(60.0));
                        ui.end_row();
                    });

                ui.add_space(16.0);
                
                ui.horizontal(|ui| {
                    let btn_rect = ui.allocate_exact_size(Vec2::new(100.0, 28.0), egui::Sense::click());
                    let fill = if btn_rect.1.hovered() { SURFACE_HOVER } else { BG };
                    ui.painter().rect(
                        btn_rect.0,
                        CornerRadius::same(6),
                        fill,
                        Stroke::new(1.0, ACCENT.linear_multiply(0.4)),
                        StrokeKind::Outside,
                    );
                    ui.painter().text(
                        btn_rect.0.center(),
                        egui::Align2::CENTER_CENTER,
                        "Save & Apply",
                        FontId::new(12.0, FontFamily::Proportional),
                        ACCENT,
                    );
                    
                    if btn_rect.1.clicked() {
                        if let Err(e) = brido_server::config::save_overlay_settings(
                            &self.runtime_env,
                            &self.settings_openai_key,
                            &self.settings_anthropic_key,
                            &self.settings_gemini_key,
                            &self.settings_openrouter_key,
                            &self.settings_hotkey_capture,
                            &self.settings_hotkey_toggle,
                        ) {
                            self.error_text = Some(format!("Failed to save settings: {}", e));
                        } else {
                            // Update config in memory
                            self.config.openai_api_key = self.settings_openai_key.clone();
                            self.config.anthropic_api_key = self.settings_anthropic_key.clone();
                            self.config.gemini_api_key = self.settings_gemini_key.clone();
                            self.config.openrouter_api_key = self.settings_openrouter_key.clone();
                            self.config.overlay_hotkey_capture = self.settings_hotkey_capture.clone();
                            self.config.overlay_hotkey_toggle = self.settings_hotkey_toggle.clone();

                            // Restart hotkey listener
                            if let Some(h) = self.hotkey_handle.take() {
                                h.stop();
                            }
                            let (_jh, new_handle) = super::hotkey::start_hotkey_listener(
                                self.hotkey_tx.clone(),
                                &self.config.overlay_hotkey_capture,
                                &self.config.overlay_hotkey_toggle,
                            );
                            self.hotkey_handle = Some(new_handle);
                            
                            self.show_settings = false;
                            self.error_text = None;
                            self.status_text = "Settings saved & applied".to_string();
                        }
                    }

                    ui.add_space(8.0);

                    let cancel_rect = ui.allocate_exact_size(Vec2::new(70.0, 28.0), egui::Sense::click());
                    let fill_cancel = if cancel_rect.1.hovered() { SURFACE_HOVER } else { BG };
                    ui.painter().rect(
                        cancel_rect.0,
                        CornerRadius::same(6),
                        fill_cancel,
                        Stroke::new(1.0, TEXT_DIM.linear_multiply(0.4)),
                        StrokeKind::Outside,
                    );
                    ui.painter().text(
                        cancel_rect.0.center(),
                        egui::Align2::CENTER_CENTER,
                        "Cancel",
                        FontId::new(12.0, FontFamily::Proportional),
                        TEXT_DIM,
                    );
                    
                    if cancel_rect.1.clicked() {
                        self.show_settings = false;
                        
                        // Revert form values to match active config on cancel
                        self.settings_openai_key = self.config.openai_api_key.clone();
                        self.settings_anthropic_key = self.config.anthropic_api_key.clone();
                        self.settings_gemini_key = self.config.gemini_api_key.clone();
                        self.settings_openrouter_key = self.config.openrouter_api_key.clone();
                        self.settings_hotkey_capture = self.config.overlay_hotkey_capture.clone();
                        self.settings_hotkey_toggle = self.config.overlay_hotkey_toggle.clone();
                    }
                });
            });
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_first_frame_setup(ctx);
        self.poll_hotkeys(ctx);
        self.poll_results();

        // Repaint frequently while loading, otherwise on events
        if self.is_loading {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let panel_frame = egui::Frame::new()
            .fill(BG)
            .inner_margin(Margin::same(16));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                // ── Drag region (title bar replacement) ──────────────
                let drag_rect = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 28.0),
                    egui::Sense::drag(),
                );
                let drag_response = drag_rect.1;

                // Draw drag handle
                let drag_r = drag_rect.0;
                ui.painter().rect_filled(drag_r, CornerRadius::ZERO, BG);

                // Title
                ui.painter().text(
                    drag_r.left_center() + egui::vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "brido overlay",
                    FontId::new(13.0, FontFamily::Proportional),
                    TEXT_DIM,
                );

                // Drag handle dots
                let dots_center = drag_r.center();
                for i in -2..=2i32 {
                    let x = dots_center.x + i as f32 * 6.0;
                    ui.painter()
                        .circle_filled(egui::pos2(x, dots_center.y), 2.0, TEXT_DIM);
                }

                // Gear Icon
                let gear_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-28.0, 2.0),
                    Vec2::new(24.0, 24.0),
                );
                let gear_resp = ui.interact(gear_rect, ui.id().with("gear"), egui::Sense::click());
                ui.painter().text(
                    gear_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "⚙",
                    FontId::new(16.0, FontFamily::Proportional),
                    if gear_resp.hovered() { TEXT_PRIMARY } else { TEXT_DIM },
                );
                if gear_resp.clicked() {
                    self.show_settings = !self.show_settings;
                }

                if drag_response.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.add_space(4.0);

                // ── Status line ──────────────────────────────────────
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if self.is_loading {
                                let dots = match (ctx.cumulative_pass_nr() / 15) % 4 {
                                    0 => ".",
                                    1 => "..",
                                    2 => "...",
                                    _ => "",
                                };
                                ui.label(
                                    RichText::new(format!("Analysing{dots}"))
                                        .color(YELLOW)
                                        .size(12.0),
                                );
                            } else {
                                let color = if self.error_text.is_some() {
                                    RED
                                } else if self.model_used.is_empty() {
                                    TEXT_SECONDARY
                                } else {
                                    ACCENT
                                };
                                ui.label(
                                    RichText::new(&self.status_text).color(color).size(12.0),
                                );
                            }
                        });
                    });

                ui.add_space(8.0);

                // ── Error display ────────────────────────────────────
                if let Some(err) = &self.error_text {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(40, 20, 20))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(RichText::new(err).color(RED).size(11.0));
                        });
                    ui.add_space(8.0);
                }

                if self.show_settings {
                    self.render_settings(ui);
                } else {
                    // ── Response area ────────────────────────────────────
                    let response_height = ui.available_height() - 80.0; // Reserve space for input
                    egui::Frame::new()
                        .fill(SURFACE)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            let mut scroll = ScrollArea::vertical()
                                .max_height(response_height)
                                .auto_shrink([false, false]);

                            if self.scroll_to_bottom {
                                scroll = scroll.stick_to_bottom(true);
                                self.scroll_to_bottom = false;
                            }

                            scroll.show(ui, |ui| {
                                if self.response_text.is_empty() && self.error_text.is_none() {
                                    ui.label(
                                        RichText::new(&format!("Press Ctrl+Shift+{} to capture & analyse\nor type a question below", self.config.overlay_hotkey_capture.to_uppercase()))
                                            .color(TEXT_DIM)
                                            .size(13.0),
                                    );
                                } else {
                                    render_response(ui, &self.response_text);
                                }
                            });
                        });

                    ui.add_space(8.0);

                    // ── Input area ───────────────────────────────────────
                    ui.horizontal(|ui| {
                        let input_width = ui.available_width() - 60.0;
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.input_text)
                                .hint_text("Ask a question…")
                                .desired_width(input_width)
                                .font(FontId::new(13.0, FontFamily::Proportional)),
                        );

                        // Submit on Enter
                        if response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.input_text.trim().is_empty()
                        {
                            self.submit_question();
                        }

                        let btn_rect = ui.allocate_exact_size(Vec2::new(52.0, 28.0), egui::Sense::click());
                        let fill = if btn_rect.1.hovered() { SURFACE_HOVER } else { SURFACE };
                        ui.painter().rect(
                            btn_rect.0,
                            CornerRadius::same(6),
                            fill,
                            Stroke::new(1.0, ACCENT.linear_multiply(0.4)),
                            StrokeKind::Outside,
                        );
                        ui.painter().text(
                            btn_rect.0.center(),
                            egui::Align2::CENTER_CENTER,
                            "Send",
                            FontId::new(12.0, FontFamily::Proportional),
                            ACCENT,
                        );
                        if btn_rect.1.clicked() && !self.input_text.trim().is_empty() {
                            self.submit_question();
                        }
                    });

                    // ── Shortcut hints ───────────────────────────────────
                    ui.add_space(4.0);
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.label(
                            RichText::new(&format!("Ctrl+Shift+{} hide  •  Ctrl+Shift+{} capture", 
                                self.config.overlay_hotkey_toggle.to_uppercase(), 
                                self.config.overlay_hotkey_capture.to_uppercase()))
                                .color(TEXT_DIM)
                                .size(10.0),
                        );
                    });
                }
            });
    }
}

/// Render response text with basic code block detection.
fn render_response(ui: &mut egui::Ui, text: &str) {
    let mut in_code_block = false;
    let mut code_buf = String::new();

    for line in text.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End of code block — render accumulated code
                egui::Frame::new()
                    .fill(CODE_BG)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&code_buf)
                                .font(FontId::new(12.0, FontFamily::Monospace))
                                .color(TEXT_PRIMARY),
                        );
                    });
                code_buf.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
        } else if in_code_block {
            if !code_buf.is_empty() {
                code_buf.push('\n');
            }
            code_buf.push_str(line);
        } else {
            // Regular text line
            if line.starts_with("Answer:") || line.starts_with("Question:") {
                ui.label(RichText::new(line).color(ACCENT).size(13.0));
            } else if line.starts_with("Why:") || line.starts_with("Explanation:") {
                ui.label(RichText::new(line).color(TEXT_SECONDARY).size(13.0));
            } else {
                ui.label(RichText::new(line).color(TEXT_PRIMARY).size(13.0));
            }
        }
    }

    // If code block was never closed, render what we have
    if in_code_block && !code_buf.is_empty() {
        egui::Frame::new()
            .fill(CODE_BG)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&code_buf)
                        .font(FontId::new(12.0, FontFamily::Monospace))
                        .color(TEXT_PRIMARY),
                );
            });
    }
}
