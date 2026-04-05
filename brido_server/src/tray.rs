use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Open,
    Quit,
}

pub struct TrayIconManager {
    tray_icon: TrayIcon,
    menu: Menu,
    open_item: MenuItem,
    quit_item: MenuItem,
    event_rx: Receiver<TrayEvent>,
    repaint_context: Arc<Mutex<Option<egui::Context>>>,
}

impl TrayIconManager {
    pub fn new(icon_rgba: Vec<u8>, width: u32, height: u32) -> Self {
        fn enqueue_event(
            tx: &std::sync::mpsc::Sender<TrayEvent>,
            repaint_context: &Arc<Mutex<Option<egui::Context>>>,
            event: TrayEvent,
        ) {
            if let Ok(guard) = repaint_context.lock() {
                if let Some(ctx) = guard.as_ref() {
                    if event == TrayEvent::Open {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    ctx.request_repaint();
                }
            }
            let _ = tx.send(event);
        }

        let icon = Icon::from_rgba(icon_rgba, width, height).expect("failed to load tray icon");
        let (event_tx, event_rx) = mpsc::channel::<TrayEvent>();
        let repaint_context = Arc::new(Mutex::new(None));

        let menu = Menu::new();
        let open_item = MenuItem::with_id("brido_tray_open", "Open", true, None);
        let quit_item = MenuItem::with_id("brido_tray_quit", "Quit", true, None);
        menu.append_items(&[&open_item, &quit_item])
            .expect("failed to build tray menu");

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Brido Server")
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .with_icon(icon)
            .build()
            .expect("failed to create tray icon");

        let open_item_id = open_item.id().clone();
        let quit_item_id = quit_item.id().clone();
        let tray_id = tray_icon.id().clone();

        let router_tx = event_tx;
        let router_repaint_context = Arc::clone(&repaint_context);
        std::thread::spawn(move || {
            loop {
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id() == &open_item_id {
                        enqueue_event(&router_tx, &router_repaint_context, TrayEvent::Open);
                    } else if event.id() == &quit_item_id {
                        enqueue_event(&router_tx, &router_repaint_context, TrayEvent::Quit);
                    }
                }

                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if event.id() != &tray_id {
                        continue;
                    }

                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        enqueue_event(&router_tx, &router_repaint_context, TrayEvent::Open);
                    }
                }

                std::thread::sleep(Duration::from_millis(16));
            }
        });

        Self {
            tray_icon,
            menu,
            open_item,
            quit_item,
            event_rx,
            repaint_context,
        }
    }

    pub fn set_repaint_context(&self, ctx: &egui::Context) {
        if let Ok(mut guard) = self.repaint_context.lock() {
            if guard.is_none() {
                *guard = Some(ctx.clone());
            }
        }
    }

    pub fn poll_events(&self) -> Option<TrayEvent> {
        let _menu_keepalive = &self.menu;
        let _items_keepalive = (&self.open_item, &self.quit_item, &self.tray_icon);
        self.event_rx.try_recv().ok()
    }
}

pub fn load_default_icon_rgba() -> (Vec<u8>, u32, u32) {
    let ico_bytes = include_bytes!("../../brido.ico");
    if let Ok(img) = image::load_from_memory_with_format(ico_bytes, image::ImageFormat::Ico) {
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        return (rgba.into_raw(), w, h);
    }

    let png_bytes = include_bytes!("../../brido.png");
    if let Ok(img) = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png) {
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        return (rgba.into_raw(), w, h);
    }

    // Safety fallback in case assets fail to decode.
    (vec![255, 255, 255, 255], 1, 1)
}
