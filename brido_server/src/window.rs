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
    /// Interim streaming text chunk for real-time display.
    StreamChunk(String),
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
    direct_type_active: bool,
    saved_position: Option<egui::Pos2>,
    scroll_to_bottom: bool,
    /// HWND of the window that had focus before the overlay took it.
    /// Used to restore focus after text input, defeating tab-switch detection.
    prev_foreground_hwnd: isize,

    // ── Settings UI state ───────────────────────────────────────────────
    show_settings: bool,
    settings_gemini_key: String,
    settings_openrouter_key: String,
    settings_ollama_key: String,
    settings_ollama_base_url: String,
    settings_deepgram_key: String,
    settings_resume: String,
    settings_jd: String,
    settings_active_provider: String,
    settings_asr_model: String,
    settings_models: std::collections::HashMap<brido_server::config::ProviderKind, String>,
    settings_hotkey_capture: String,
    settings_hotkey_toggle: String,
    settings_hotkey_settings: String,
    settings_hotkey_stealth: String,
    settings_hotkey_direct_type: String,
    settings_strict_stealth_mode: bool,

    show_password_gemini: bool,
    show_password_openrouter: bool,
    start_on_startup: bool,
    minimize_to_tray: bool,
    editing_hotkey: Option<String>,
    hotkey_edit_buffer: String,

    // ── Phone Connection state ───────────────────────────────────────────
    ip: String,
    pin: String,
    port: u16,
    server_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
    connected_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    show_qr: bool,
    qr_texture: Option<egui::TextureHandle>,
    voice_mode: bool,
    voice_copilot_task: Option<tokio::task::JoinHandle<()>>,
    audio_stop_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl OverlayApp {
    pub fn new(
        hotkey_tx: mpsc::Sender<OverlayEvent>,
        hotkey_rx: mpsc::Receiver<OverlayEvent>,
        hotkey_handle: super::hotkey::HotkeyHandle,
        rt: tokio::runtime::Handle,
        config: brido_server::config::Config,
        runtime_env: brido_server::config::RuntimeEnvPaths,
        ip: String,
        pin: String,
        port: u16,
        server_ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
        connected_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        let (result_tx, result_rx) = mpsc::channel();
        Self {
            hotkey_tx,
            hotkey_rx,
            hotkey_handle: Some(hotkey_handle),
            result_rx,
            result_tx,
            rt,
            settings_gemini_key: config.gemini_api_key.clone(),
            settings_openrouter_key: config.openrouter_api_key.clone(),
            settings_ollama_key: config.ollama_api_key.clone(),
            settings_ollama_base_url: config.ollama_base_url.clone(),
            settings_deepgram_key: config.deepgram_api_key.clone(),
            settings_resume: config.resume_text.clone(),
            settings_jd: config.job_description_text.clone(),
            settings_active_provider: config.active_provider.clone(),
            settings_asr_model: config.asr_model.clone(),
            settings_models: {
                let mut map = std::collections::HashMap::new();
                map.insert(brido_server::config::ProviderKind::Gemini, config.gemini_model.clone());
                map.insert(brido_server::config::ProviderKind::OpenRouter, config.openrouter_model.clone());
                map.insert(brido_server::config::ProviderKind::Ollama, config.ollama_model.clone());
                map
            },
            settings_hotkey_capture: strip_ctrl(&config.overlay_hotkey_capture),
            settings_hotkey_toggle: strip_ctrl(&config.overlay_hotkey_toggle),
            settings_hotkey_settings: strip_ctrl(&config.overlay_hotkey_settings),
            settings_hotkey_stealth: strip_ctrl(&config.overlay_hotkey_stealth),
            settings_hotkey_direct_type: strip_ctrl(&config.overlay_hotkey_direct_type),
            settings_strict_stealth_mode: config.strict_stealth_mode,
            show_password_gemini: false,
            show_password_openrouter: false,
            start_on_startup: false,
            minimize_to_tray: false,
            editing_hotkey: None,
            hotkey_edit_buffer: String::new(),
            config,
            runtime_env,
            response_text: String::new(),
            model_used: String::new(),
            status_text: "weLcome!!!".to_string(),
            input_text: String::new(),
            is_loading: false,
            is_visible: true,
            error_text: None,
            stealth_applied: false,
            direct_type_active: false,
            saved_position: None,
            scroll_to_bottom: false,
            prev_foreground_hwnd: 0,
            show_settings: false,
            ip,
            pin,
            port,
            server_ready,
            connected_count,
            show_qr: false,
            qr_texture: None,
            voice_mode: false,
            voice_copilot_task: None,
            audio_stop_tx: None,
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

    fn toggle_voice_mode(&mut self) {
        self.status_text = "In development".to_string();
    }

    /// Submit a manual text question (captures screen as context).
    /// After submission, restores focus to the previous window to avoid
    /// triggering browser tab-switching detection.
    fn submit_question(&mut self) {
        let question = self.input_text.trim().to_string();
        if question.is_empty() {
            return;
        }
        self.input_text.clear();
        self.trigger_capture(Some(question));

        if self.direct_type_active {
            self.direct_type_active = false;
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
            unsafe {
                let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                    if !hwnd.is_invalid() {
                        if self.config.strict_stealth_mode {
                            super::stealth::disable_typing(hwnd.0 as isize);
                        }
                    }
                }
            }
        }

        // Restore focus to the browser/previous window immediately
        super::stealth::restore_focus(self.prev_foreground_hwnd);
        self.prev_foreground_hwnd = 0;
    }

    /// Process pending hotkey events.
    fn poll_hotkeys(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.hotkey_rx.try_recv() {
            match event {
                OverlayEvent::CaptureAndAnalyse => {
                    self.is_visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    self.trigger_capture(None);
                }

                OverlayEvent::ToggleStealth => {
                    self.config.strict_stealth_mode = !self.config.strict_stealth_mode;
                    self.settings_strict_stealth_mode = self.config.strict_stealth_mode;
                    use windows::core::PCWSTR;
                    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
                    unsafe {
                        let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                            if !hwnd.is_invalid() {
                                if self.config.strict_stealth_mode {
                                    super::stealth::apply_stealth(hwnd.0 as isize);
                                    self.status_text = "Stealth Mode ON".to_string();
                                } else {
                                    super::stealth::remove_stealth(hwnd.0 as isize);
                                    self.status_text = "Stealth Mode OFF".to_string();
                                }
                            }
                        }
                    }
                    // Persist toggle state
                    if let Err(e) = brido_server::config::save_overlay_settings(
                        &self.runtime_env,
                        &self.config.active_provider,
                        &self.config.asr_model,
                        &self.config.gemini_api_key,
                        &self.config.openrouter_api_key,
                        &self.config.ollama_api_key,
                        &self.config.ollama_base_url,
                        &self.config.deepgram_api_key,
                        &self.config.resume_text,
                        &self.config.job_description_text,
                        &self.config.overlay_hotkey_capture,
                        &self.config.overlay_hotkey_toggle,
                        &self.config.overlay_hotkey_settings,
                        &self.config.overlay_hotkey_stealth,
                        &self.config.overlay_hotkey_direct_type,
                        self.config.strict_stealth_mode,
                        &self.settings_models,
                    ) {
                        tracing::error!("Failed to save stealth mode toggle: {}", e);
                    }
                    ctx.request_repaint();
                }

                OverlayEvent::DirectType => {
                    self.direct_type_active = !self.direct_type_active;
                    self.is_visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

                    use windows::core::PCWSTR;
                    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
                    unsafe {
                        let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                            if !hwnd.is_invalid() {
                                if self.direct_type_active {
                                    if self.config.strict_stealth_mode {
                                        super::stealth::enable_typing(hwnd.0 as isize);
                                    }
                                    // Save foreground window before taking focus
                                    let fg = super::stealth::get_foreground_window();
                                    if fg != 0 {
                                        self.prev_foreground_hwnd = fg;
                                    }
                                } else {
                                    if self.config.strict_stealth_mode {
                                        super::stealth::disable_typing(hwnd.0 as isize);
                                    }
                                    super::stealth::restore_focus(self.prev_foreground_hwnd);
                                }
                            }
                        }
                    }
                }

                OverlayEvent::ToggleVisibility => {
                    self.is_visible = !self.is_visible;
                    if self.is_visible {
                        // Restore: bring window back to saved position
                        if let Some(pos) = self.saved_position.take() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                                pos.into(),
                            ));
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    } else {
                        // Hide: save current position, then move offscreen
                        // We read the current viewport position if available
                        let screen = ctx.input(|i| i.screen_rect);
                        // Use last known position from apply_first_frame_setup as fallback
                        let current_pos = ctx.input(|i| {
                            i.viewport().outer_rect.map(|r| r.min)
                        }).unwrap_or(egui::pos2(screen.max.x - 380.0 - 12.0, 12.0));
                        self.saved_position = Some(current_pos);
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                            egui::pos2(-10000.0, -10000.0).into(),
                        ));
                    }
                    ctx.request_repaint();
                }
                OverlayEvent::OpenSettings => {
                    self.show_settings = !self.show_settings;
                    self.show_qr = false;
                    if self.show_settings && !self.is_visible {
                        self.is_visible = true;
                        if let Some(pos) = self.saved_position.take() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                                pos.into(),
                            ));
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    }
                    ctx.request_repaint();
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
                AnalysisResult::StreamChunk(chunk) => {
                    if self.model_used == "Deepgram ASR" || self.response_text.is_empty() {
                        // First chunk of an AI response, overwrite the transcript
                        self.response_text = chunk;
                        self.model_used = "Voice Copilot".to_string();
                        self.status_text = "✓ Voice Copilot (streaming)".to_string();
                    } else {
                        // Append subsequent chunks
                        self.response_text.push_str(&chunk);
                    }
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
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.heading(RichText::new("Settings").color(TEXT_PRIMARY));
                        ui.add_space(8.0);
                
                ui.label(RichText::new("Active AI Settings").color(TEXT_SECONDARY));
                ui.add_space(4.0);

                let mut current_kind = brido_server::config::ProviderKind::from_label(&self.settings_active_provider).unwrap_or(brido_server::config::ProviderKind::Gemini);
                
                egui::Grid::new("provider_picker_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Provider:").color(TEXT_PRIMARY));
                        egui::ComboBox::from_id_salt("provider_picker")
                            .selected_text(current_kind.label())
                            .show_ui(ui, |ui| {
                                for kind in brido_server::config::ProviderKind::ALL {
                                    if ui.selectable_label(current_kind == kind, kind.label()).clicked() {
                                        self.settings_active_provider = kind.label().to_string();
                                        current_kind = kind;
                                        
                                        // Set default model for the new provider if it's not present in settings_models
                                        if !self.settings_models.contains_key(&current_kind) {
                                            self.settings_models.insert(current_kind, current_kind.default_model().to_string());
                                        }
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label(RichText::new("Model:").color(TEXT_PRIMARY));
                        let mut current_model = self.settings_models.get(&current_kind).cloned().unwrap_or_else(|| current_kind.default_model().to_string());
                        let models = current_kind.available_models();
                        
                        ui.horizontal(|ui| {
                            let is_custom = !models.contains(&current_model.as_str());
                            let display_text = if is_custom { "Custom..." } else { &current_model };
                            
                            egui::ComboBox::from_id_salt("model_picker")
                                .selected_text(display_text)
                                .show_ui(ui, |ui| {
                                    for m in &models {
                                        if ui.selectable_label(!is_custom && current_model == *m, *m).clicked() {
                                            current_model = m.to_string();
                                            self.settings_models.insert(current_kind, current_model.clone());
                                        }
                                    }
                                    if ui.selectable_label(is_custom, "Custom...").clicked() {
                                        if !is_custom {
                                            current_model = "".to_string();
                                            self.settings_models.insert(current_kind, current_model.clone());
                                        }
                                    }
                                });

                            if !models.contains(&current_model.as_str()) {
                                if ui.add(egui::TextEdit::singleline(&mut current_model).desired_width(120.0)).changed() {
                                    self.settings_models.insert(current_kind, current_model.clone());
                                }
                            }
                        });
                        ui.end_row();



                        ui.label(RichText::new("Voice Model:").color(TEXT_PRIMARY));
                        egui::ComboBox::from_id_salt("asr_model_picker")
                            .selected_text(&self.settings_asr_model)
                            .show_ui(ui, |ui| {
                                let models = ["nova-3", "nova-2"];
                                for m in models {
                                    ui.selectable_value(&mut self.settings_asr_model, m.to_string(), m);
                                }
                            });
                        ui.end_row();


                    });

                ui.add_space(12.0);
                ui.label(RichText::new("API Keys").color(TEXT_SECONDARY));
                ui.add_space(4.0);

                egui::Grid::new("settings_api_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Gemini:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_gemini_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("OpenRouter:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_openrouter_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("Ollama (local):").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_ollama_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("Ollama URL:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_ollama_base_url));
                        ui.end_row();



                        ui.label(RichText::new("Deepgram API:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::singleline(&mut self.settings_deepgram_key).password(true));
                        ui.end_row();

                        ui.label(RichText::new("Resume:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::multiline(&mut self.settings_resume).desired_rows(3));
                        ui.end_row();

                        ui.label(RichText::new("Job Description:").color(TEXT_PRIMARY));
                        ui.add(egui::TextEdit::multiline(&mut self.settings_jd).desired_rows(3));
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.checkbox(&mut self.settings_strict_stealth_mode, "Strict (Stealth) Mode");

                ui.add_space(12.0);
                ui.label(RichText::new("Hotkeys").color(TEXT_SECONDARY));
                ui.add_space(4.0);
                
                egui::Grid::new("settings_hotkey_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Capture:").color(TEXT_PRIMARY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Ctrl +").color(TEXT_DIM));
                            ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_capture).desired_width(100.0));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Toggle/Hide:").color(TEXT_PRIMARY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Ctrl +").color(TEXT_DIM));
                            ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_toggle).desired_width(100.0));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Toggle Stealth:").color(TEXT_PRIMARY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Ctrl +").color(TEXT_DIM));
                            ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_stealth).desired_width(100.0));
                        });
                        ui.end_row();
                        ui.label(RichText::new("Direct Type:").color(TEXT_PRIMARY));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Ctrl +").color(TEXT_DIM));
                            ui.add(egui::TextEdit::singleline(&mut self.settings_hotkey_direct_type).desired_width(100.0));
                        });
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
                        let cap_suffix = self.settings_hotkey_capture.trim().to_uppercase();
                        let tog_suffix = self.settings_hotkey_toggle.trim().to_uppercase();
                        
                        // We also need to get settings_hotkey_settings to uppercase but since it isn't shown in UI currently we just use it
                        let set_suffix = self.settings_hotkey_settings.trim().to_uppercase();

                        if !is_valid_hotkey_suffix(&cap_suffix) {
                            self.error_text = Some("Invalid Capture hotkey. Use A-Z, 0-9, Space, or basic punctuation. No modifiers (Shift/Alt/Enter).".to_string());
                        } else if !is_valid_hotkey_suffix(&tog_suffix) {
                            self.error_text = Some("Invalid Toggle hotkey. Use A-Z, 0-9, Space, or basic punctuation. No modifiers (Shift/Alt/Enter).".to_string());
                        } else {
                            let capture_full = format!("Ctrl+{}", cap_suffix);
                            let toggle_full = format!("Ctrl+{}", tog_suffix);
                            let settings_full = format!("Ctrl+{}", set_suffix);
                            let stealth_full = format!("Ctrl+{}", strip_ctrl(&self.settings_hotkey_stealth));
                            let direct_type_full = format!("Ctrl+{}", strip_ctrl(&self.settings_hotkey_direct_type));

                            if let Err(e) = brido_server::config::save_overlay_settings(
                                &self.runtime_env,
                                &self.settings_active_provider,
                                &self.settings_asr_model,
                                &self.settings_gemini_key,
                                &self.settings_openrouter_key,
                                &self.settings_ollama_key,
                                &self.settings_ollama_base_url,
                                &self.settings_deepgram_key,
                                &self.settings_resume,
                                &self.settings_jd,
                                &capture_full,
                                &toggle_full,
                                &settings_full,
                                &stealth_full,
                                &direct_type_full,
                                self.settings_strict_stealth_mode,
                                &self.settings_models,
                            ) {
                                self.error_text = Some(format!("Failed to save settings: {}", e));
                            } else {
                                // Update config in memory
                                self.config.gemini_api_key = self.settings_gemini_key.clone();
                                self.config.openrouter_api_key = self.settings_openrouter_key.clone();
                                self.config.ollama_api_key = self.settings_ollama_key.clone();
                                self.config.ollama_base_url = self.settings_ollama_base_url.clone();
                                self.config.deepgram_api_key = self.settings_deepgram_key.clone();
                                self.config.resume_text = self.settings_resume.clone();
                                self.config.job_description_text = self.settings_jd.clone();
                                self.config.active_provider = self.settings_active_provider.clone();
                                self.config.asr_model = self.settings_asr_model.clone();
                                if let Some(m) = self.settings_models.get(&brido_server::config::ProviderKind::Gemini) { self.config.gemini_model = m.clone(); }
                                if let Some(m) = self.settings_models.get(&brido_server::config::ProviderKind::OpenRouter) { self.config.openrouter_model = m.clone(); }
                                if let Some(m) = self.settings_models.get(&brido_server::config::ProviderKind::Ollama) { self.config.ollama_model = m.clone(); }
                                
                                self.config.overlay_hotkey_capture = capture_full;
                                self.config.overlay_hotkey_toggle = toggle_full;
                                self.config.overlay_hotkey_settings = settings_full;
                                self.config.overlay_hotkey_stealth = stealth_full;
                                self.config.overlay_hotkey_direct_type = direct_type_full;
                                self.config.strict_stealth_mode = self.settings_strict_stealth_mode;
                                
                                if !self.config.strict_stealth_mode {
                                    use windows::core::PCWSTR;
                                    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
                                    unsafe {
                                        let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                                        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                                            if !hwnd.is_invalid() {
                                                super::stealth::remove_stealth(hwnd.0 as isize);
                                            }
                                        }
                                    }
                                } else {
                                    use windows::core::PCWSTR;
                                    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
                                    unsafe {
                                        let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                                        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                                            if !hwnd.is_invalid() {
                                                super::stealth::apply_stealth(hwnd.0 as isize);
                                            }
                                        }
                                    }
                                }

                                // Restart hotkey listener
                                if let Some(h) = self.hotkey_handle.take() {
                                    h.stop();
                                }
                                let (_jh, new_handle) = super::hotkey::start_hotkey_listener(
                                    self.hotkey_tx.clone(),
                                    &self.config.overlay_hotkey_capture,
                                    &self.config.overlay_hotkey_toggle,
                                    &self.config.overlay_hotkey_settings,
                                    &self.config.overlay_hotkey_stealth,
                                    &self.config.overlay_hotkey_direct_type,
                                );
                                self.hotkey_handle = Some(new_handle);
                                
                                self.show_settings = false;
                                self.error_text = None;
                                self.status_text = "Settings saved & applied".to_string();
                            }
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
                        self.settings_gemini_key = self.config.gemini_api_key.clone();
                        self.settings_openrouter_key = self.config.openrouter_api_key.clone();
                        self.settings_ollama_key = self.config.ollama_api_key.clone();
                        self.settings_ollama_base_url = self.config.ollama_base_url.clone();
                        self.settings_deepgram_key = self.config.deepgram_api_key.clone();
                        self.settings_resume = self.config.resume_text.clone();
                        self.settings_jd = self.config.job_description_text.clone();
                        self.settings_active_provider = self.config.active_provider.clone();
                        self.settings_asr_model = self.config.asr_model.clone();
                        self.settings_models.insert(brido_server::config::ProviderKind::Gemini, self.config.gemini_model.clone());
                        self.settings_models.insert(brido_server::config::ProviderKind::OpenRouter, self.config.openrouter_model.clone());
                        self.settings_models.insert(brido_server::config::ProviderKind::Ollama, self.config.ollama_model.clone());
                        
                        self.settings_hotkey_capture = strip_ctrl(&self.config.overlay_hotkey_capture);
                        self.settings_hotkey_toggle = strip_ctrl(&self.config.overlay_hotkey_toggle);
                        self.settings_hotkey_settings = strip_ctrl(&self.config.overlay_hotkey_settings);
                        self.settings_hotkey_stealth = strip_ctrl(&self.config.overlay_hotkey_stealth);
                        self.settings_hotkey_direct_type = strip_ctrl(&self.config.overlay_hotkey_direct_type);
                        self.settings_strict_stealth_mode = self.config.strict_stealth_mode;
                        self.error_text = None;
                    }
                });
            });
        });
    }

    fn render_qr(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(SURFACE)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.heading(RichText::new("Phone Connection").color(TEXT_PRIMARY));
                ui.add_space(8.0);
                
                let is_ready = self.server_ready.load(std::sync::atomic::Ordering::Relaxed);
                
                if !is_ready {
                    ui.label(RichText::new("Starting server...").color(YELLOW));
                    return;
                }
                
                let payload = format!("brido://{}:{}:{}", self.ip, self.port, self.pin);
                
                if self.qr_texture.is_none() {
                    self.qr_texture = Some(crate::ui::qr_panel::generate_qr_texture(ctx, &payload, None));
                }
                
                if let Some(tex) = &self.qr_texture {
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(160.0, 160.0)));
                    });
                }
                
                ui.add_space(8.0);
                ui.label(RichText::new(format!("IP: {}", self.ip)).color(TEXT_PRIMARY));
                ui.label(RichText::new(format!("Port: {}", self.port)).color(TEXT_PRIMARY));
                ui.label(RichText::new(format!("PIN: {}", self.pin)).color(ACCENT));
                
                ui.add_space(8.0);
                let conns = self.connected_count.load(std::sync::atomic::Ordering::Relaxed);
                if conns > 0 {
                    ui.label(RichText::new(format!("{} device(s) connected", conns)).color(ACCENT));
                } else {
                    ui.label(RichText::new("Waiting for connection...").color(TEXT_DIM));
                }
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
                    egui::Sense::hover(),
                );
                let drag_response = drag_rect.1;

                // Draw drag handle
                let drag_r = drag_rect.0;
                ui.painter().rect_filled(drag_r, CornerRadius::ZERO, BG);

                // Title
                ui.painter().text(
                    drag_r.left_center() + egui::vec2(8.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Brido",
                    FontId::new(13.0, FontFamily::Proportional),
                    TEXT_DIM,
                );

                // Exit Button
                let exit_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-28.0, 2.0),
                    Vec2::new(24.0, 24.0),
                );
                let exit_resp = ui.interact(exit_rect, ui.id().with("exit"), egui::Sense::click());
                ui.painter().text(
                    exit_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "X",
                    FontId::new(14.0, FontFamily::Proportional),
                    if exit_resp.hovered() { Color32::from_rgb(220, 50, 50) } else { TEXT_DIM },
                );
                if exit_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Phone Icon
                let phone_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-80.0, 2.0),
                    Vec2::new(24.0, 24.0),
                );
                let phone_resp = ui.interact(phone_rect, ui.id().with("phone"), egui::Sense::click());
                ui.painter().text(
                    phone_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📱",
                    FontId::new(14.0, FontFamily::Proportional),
                    if phone_resp.hovered() { TEXT_PRIMARY } else { TEXT_DIM },
                );
                if phone_resp.clicked() {
                    self.show_qr = !self.show_qr;
                    if self.show_qr {
                        self.show_settings = false;
                    }
                }

                // Drag Icon
                let move_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-132.0, 2.0),
                    Vec2::new(24.0, 24.0),
                );
                let move_resp = ui.interact(move_rect, ui.id().with("move"), egui::Sense::drag());
                ui.painter().text(
                    move_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✋",
                    FontId::new(14.0, FontFamily::Proportional),
                    if move_resp.hovered() || move_resp.dragged() { TEXT_PRIMARY } else { TEXT_DIM },
                );
                if move_resp.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // Voice Toggle Icon
                let voice_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-106.0, 2.0),
                    Vec2::new(24.0, 24.0),
                );
                let voice_resp = ui.interact(voice_rect, ui.id().with("voice"), egui::Sense::click());
                ui.painter().text(
                    voice_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if self.voice_mode { "🎙" } else { "🎤" },
                    FontId::new(14.0, FontFamily::Proportional),
                    if voice_resp.hovered() { TEXT_PRIMARY } else if self.voice_mode { ACCENT } else { TEXT_DIM },
                );
                if voice_resp.clicked() {
                    self.toggle_voice_mode();
                }

                // Gear Icon
                let gear_rect = egui::Rect::from_min_size(
                    drag_r.right_top() + egui::vec2(-54.0, 2.0),
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
                    if self.show_settings {
                        self.show_qr = false;
                    }
                }

                // Removed overall drag response

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
                } else if self.show_qr {
                    self.render_qr(ctx, ui);
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
                                        RichText::new(&format!("Press {} to capture & analyse\nor type a question below", self.config.overlay_hotkey_capture.to_uppercase()))
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

                        if self.direct_type_active {
                            response.request_focus();
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.direct_type_active = false;
                                use windows::core::PCWSTR;
                                use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
                                unsafe {
                                    let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                                    if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
                                        if !hwnd.is_invalid() {
                                            if self.config.strict_stealth_mode {
                                                super::stealth::disable_typing(hwnd.0 as isize);
                                            }
                                            super::stealth::restore_focus(self.prev_foreground_hwnd);
                                        }
                                    }
                                }
                            }
                        }

                        // Save the foreground window when the text input gains focus
                        // so we can give it back after submission.
                        if response.gained_focus() {
                            let fg = super::stealth::get_foreground_window();
                            if fg != 0 {
                                self.prev_foreground_hwnd = fg;
                            }
                        }

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
                            // Focus restore is handled inside submit_question()
                        }
                    });

                    // ── Shortcut hints ───────────────────────────────────
                    ui.add_space(4.0);
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.label(
                            RichText::new(&format!("{} hide  •  {} capture", 
                                self.config.overlay_hotkey_toggle, 
                                self.config.overlay_hotkey_capture))
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

fn strip_ctrl(s: &str) -> String {
    let lower = s.to_lowercase();
    if let Some(stripped) = lower.strip_prefix("ctrl+") {
        // preserve the case of the remaining string
        s[s.len() - stripped.len()..].to_string()
    } else if let Some(stripped) = lower.strip_prefix("ctrl + ") {
        s[s.len() - stripped.len()..].to_string()
    } else {
        s.to_string()
    }
}

fn is_valid_hotkey_suffix(s: &str) -> bool {
    let s = s.trim().to_uppercase();
    if s.is_empty() {
        return false;
    }
    
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return false;
    }

    // Check modifiers (all parts except the last)
    // The user explicitly requested: "not adding like shift tab capslock... alt is exception"
    for &part in &parts[..parts.len() - 1] {
        if part != "ALT" {
            return false;
        }
    }

    let k = parts.last().unwrap();

    let valid_words = ["SPACE", "`", "~", "BACKTICK", "[", "]", ",", "."];
    if valid_words.contains(k) {
        return true;
    }
    
    // Reject system or special keys as the final key
    let invalid = ["SHIFT", "CTRL", "ALT", "WIN", "ENTER", "DEL", "DELETE", "TAB", "ESC", "ESCAPE", "CAPSLOCK", "CAPS"];
    if invalid.contains(k) {
        return false;
    }
    
    // Allow single characters
    if k.len() == 1 {
        let c = k.chars().next().unwrap();
        if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() {
            return true;
        }
    }
    
    false
}
