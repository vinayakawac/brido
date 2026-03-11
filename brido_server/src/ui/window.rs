use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, Layout, RichText,
    Stroke, StrokeKind, TextureHandle, Vec2,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    pub phone_connected: bool,
    header: HeaderState,
    qr_texture: Option<TextureHandle>,
    qr_payload: String,
    shutdown_flag: Arc<AtomicBool>,
    server_ready: Arc<AtomicBool>,
    connected_count: Arc<AtomicUsize>,
    axum_handle: axum_server::Handle,
}

impl BridoApp {
    pub fn new(
        ip: String,
        pin: String,
        port: u16,
        shutdown_flag: Arc<AtomicBool>,
        server_ready: Arc<AtomicBool>,
        connected_count: Arc<AtomicUsize>,
        axum_handle: axum_server::Handle,
    ) -> Self {
        let qr_payload = format!("brido://{}:{}:{}", ip, port, pin);
        Self {
            ip,
            pin,
            port,
            status: ServerStatus::Running,
            phone_connected: false,
            header: HeaderState::default(),
            qr_texture: None,
            qr_payload,
            shutdown_flag,
            server_ready,
            connected_count,
            axum_handle,
        }
    }

    fn handle_action(&mut self, action: ControlAction, ctx: &egui::Context) {
        match action {
            ControlAction::Restart => {
                self.status = ServerStatus::Starting;
                // Shut down the old server gracefully
                self.axum_handle.shutdown();
                // Create a new config (generates a new PIN)
                let new_config = crate::config::Config::default();
                self.pin = new_config.pin.clone();
                let new_ip = local_ip_address::local_ip()
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                self.ip = new_ip;
                println!("  Restarting server… new PIN: {}", self.pin);
                // Start a new server (reuses the same shared ready / count flags)
                self.axum_handle = crate::start_server(
                    new_config,
                    self.server_ready.clone(),
                    self.connected_count.clone(),
                );
                // Update QR code
                self.qr_payload = format!("brido://{}:{}:{}", self.ip, self.port, self.pin);
                self.qr_texture = None;
                self.header.reset();
            }
            ControlAction::StopServer => {
                self.status = ServerStatus::Stopped;
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
        // Request repaint during typing animation or when waiting for server
        if !self.header.is_collapsed() || self.status == ServerStatus::Starting {
            ctx.request_repaint();
        }

        // Update phone connection status from shared counter
        self.phone_connected = self.connected_count.load(Ordering::SeqCst) > 0;

        // Transition Starting → Running when the server signals ready
        if self.status == ServerStatus::Starting && self.server_ready.load(Ordering::SeqCst) {
            self.status = ServerStatus::Running;
            println!("  Server is now running.");
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
                        // Phone connected status
                        ui.vertical(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                let dot = RichText::new("■")
                                    .color(self.status.color())
                                    .size(12.0);
                                let label = RichText::new(self.status.label())
                                    .color(self.status.color())
                                    .size(12.0);
                                ui.label(label);
                                ui.label(dot);
                            });
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                let pc_color = if self.phone_connected { ACCENT } else { TEXT_SECONDARY };
                                let pc_label = if self.phone_connected { "phone connected" } else { "phone disconnected" };
                                let dot = RichText::new("■")
                                    .color(pc_color)
                                    .size(12.0);
                                let label = RichText::new(pc_label)
                                    .color(pc_color)
                                    .size(12.0);
                                ui.label(label);
                                ui.label(dot);
                            });
                        });
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
                ui.add_space(ui.available_height() - 100.0);

                // ── Control Buttons ──────────────────────────────────
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

                // Full-width restart server button
                let avail_w = ui.available_width();
                if make_btn(ui, "restart server", ACCENT, avail_w, 44.0) {
                    action = Some(ControlAction::Restart);
                }

                ui.add_space(8.0);

                // Row of 3 smaller buttons
                ui.horizontal(|ui| {
                    let btn_height = 36.0;
                    let spacing = ui.spacing().item_spacing.x;
                    let btn_width = (avail_w - spacing * 2.0) / 3.0;

                    if make_btn(ui, "stop server", TEXT_PRIMARY, btn_width, btn_height) {
                        action = Some(ControlAction::StopServer);
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
