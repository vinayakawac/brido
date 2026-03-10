use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, Layout, RichText,
    Stroke, StrokeKind, TextureHandle, Vec2,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::controls::ControlAction;
use super::header::HeaderState;
use super::qr_panel::generate_qr_texture;

// ── Colours ─────────────────────────────────────────────────────────────────
const BG: Color32 = Color32::from_rgb(18, 18, 18);
const SURFACE: Color32 = Color32::from_rgb(30, 30, 30);
const SURFACE_HOVER: Color32 = Color32::from_rgb(42, 42, 42);
const ACCENT: Color32 = Color32::from_rgb(0, 230, 118);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 240, 240);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 160, 160);
const RED: Color32 = Color32::from_rgb(239, 83, 80);
const YELLOW: Color32 = Color32::from_rgb(255, 202, 40);

// ── Server status ───────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    Starting,
    Running,
    Stopped,
}

impl ServerStatus {
    pub fn color(self) -> Color32 {
        match self {
            Self::Running => ACCENT,
            Self::Starting => YELLOW,
            Self::Stopped => RED,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "server running",
            Self::Starting => "starting…",
            Self::Stopped => "stopped",
        }
    }
}

// ── App state ───────────────────────────────────────────────────────────────
pub struct BridoApp {
    pub ip: String,
    pub pin: String,
    pub port: u16,
    pub status: ServerStatus,
    header: HeaderState,
    qr_texture: Option<TextureHandle>,
    qr_payload: String,
    shutdown_flag: Arc<AtomicBool>,
    restart_flag: Arc<AtomicBool>,
}

impl BridoApp {
    pub fn new(
        ip: String,
        pin: String,
        port: u16,
        shutdown_flag: Arc<AtomicBool>,
        restart_flag: Arc<AtomicBool>,
    ) -> Self {
        let qr_payload = format!("brido://connect?ip={}&pin={}&port={}", ip, pin, port);
        Self {
            ip,
            pin,
            port,
            status: ServerStatus::Running,
            header: HeaderState::default(),
            qr_texture: None,
            qr_payload,
            shutdown_flag,
            restart_flag,
        }
    }

    /// Regenerate QR code (after restart with new PIN / IP).
    pub fn refresh_qr(&mut self, ip: String, pin: String) {
        self.ip = ip;
        self.pin = pin;
        self.qr_payload = format!("brido://connect?ip={}&pin={}&port={}", self.ip, self.pin, self.port);
        self.qr_texture = None; // will be re-created next frame
        self.header.reset();
    }

    fn handle_action(&mut self, action: ControlAction, ctx: &egui::Context) {
        match action {
            ControlAction::Restart => {
                self.status = ServerStatus::Starting;
                self.restart_flag.store(true, Ordering::SeqCst);
            }
            ControlAction::Minimize => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            ControlAction::Shutdown => {
                self.status = ServerStatus::Stopped;
                self.shutdown_flag.store(true, Ordering::SeqCst);
            }
        }
    }
}

impl eframe::App for BridoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint during typing animation
        if !self.header.is_collapsed() {
            ctx.request_repaint();
        }

        // Lazy-init QR texture
        if self.qr_texture.is_none() {
            let tex = generate_qr_texture(ctx, &self.qr_payload, None);
            self.qr_texture = Some(tex);
        }

        // Background
        let panel_frame = egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::same(24));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                let mut action: Option<ControlAction> = None;

                // ── Header ───────────────────────────────────────────
                ui.horizontal(|ui| {
                    let text = self.header.current_text();
                    let font = if self.header.is_collapsed() {
                        FontId::new(28.0, FontFamily::Proportional)
                    } else {
                        FontId::new(22.0, FontFamily::Proportional)
                    };
                    ui.label(RichText::new(text).font(font).color(TEXT_PRIMARY));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dot = RichText::new("●")
                            .color(self.status.color())
                            .size(14.0);
                        let label = RichText::new(self.status.label())
                            .color(self.status.color())
                            .size(13.0);
                        ui.label(label);
                        ui.label(dot);
                    });
                });

                ui.add_space(20.0);

                // ── Server Info Card ─────────────────────────────────
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Connection details")
                                .color(TEXT_SECONDARY)
                                .size(12.0),
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("IP address :")
                                    .color(TEXT_SECONDARY)
                                    .size(15.0),
                            );
                            let ip_label = ui.label(
                                RichText::new(&self.ip)
                                    .color(TEXT_PRIMARY)
                                    .size(15.0),
                            );
                            if ip_label.clicked() {
                                ctx.copy_text(self.ip.clone());
                            }
                            if ip_label.hovered() {
                                ui.label(
                                    RichText::new("(click to copy)")
                                        .color(TEXT_SECONDARY)
                                        .size(11.0),
                                );
                            }
                        });

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("PIN        :")
                                    .color(TEXT_SECONDARY)
                                    .size(15.0),
                            );
                            let pin_label = ui.label(
                                RichText::new(&self.pin)
                                    .color(ACCENT)
                                    .size(15.0),
                            );
                            if pin_label.clicked() {
                                ctx.copy_text(self.pin.clone());
                            }
                            if pin_label.hovered() {
                                ui.label(
                                    RichText::new("(click to copy)")
                                        .color(TEXT_SECONDARY)
                                        .size(11.0),
                                );
                            }
                        });
                    });

                ui.add_space(16.0);

                // ── QR Code Panel ────────────────────────────────────
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Scan to connect")
                                    .color(TEXT_SECONDARY)
                                    .size(12.0),
                            );
                            ui.add_space(8.0);

                            if let Some(tex) = &self.qr_texture {
                                let size = Vec2::splat(200.0);
                                ui.image(egui::load::SizedTexture::new(tex.id(), size));
                            }
                        });
                    });

                // Fill remaining space before buttons
                ui.add_space(ui.available_height() - 56.0);

                // ── Control Buttons ──────────────────────────────────
                ui.horizontal(|ui| {
                    let btn_height = 44.0;
                    let avail = ui.available_width();
                    let spacing = ui.spacing().item_spacing.x;
                    let btn_width = (avail - spacing * 2.0) / 3.0;

                    let make_btn =
                        |ui: &mut egui::Ui, label: &str, color: Color32, w: f32, h: f32| -> bool {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(w, h),
                                egui::Sense::click(),
                            );
                            let fill = if response.hovered() { SURFACE_HOVER } else { SURFACE };
                            ui.painter().rect(
                                rect,
                                CornerRadius::same(8),
                                fill,
                                Stroke::new(1.0, color.linear_multiply(0.4)),
                                StrokeKind::Outside,
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                label,
                                FontId::new(14.0, FontFamily::Proportional),
                                color,
                            );
                            response.clicked()
                        };

                    if make_btn(ui, "restart server", ACCENT, btn_width, btn_height) {
                        action = Some(ControlAction::Restart);
                    }
                    if make_btn(ui, "minimize", TEXT_PRIMARY, btn_width, btn_height) {
                        action = Some(ControlAction::Minimize);
                    }
                    if make_btn(ui, "shutdown", RED, btn_width, btn_height) {
                        action = Some(ControlAction::Shutdown);
                    }
                });

                if let Some(a) = action {
                    self.handle_action(a, ctx);
                }
            });

        // If shutdown was requested, close the window
        if self.shutdown_flag.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
