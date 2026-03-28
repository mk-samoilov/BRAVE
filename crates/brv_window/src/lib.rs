use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{CursorGrabMode, Fullscreen, WindowBuilder},
};

pub use winit::window::Window as WinitWindow;

pub struct WindowConfig {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

pub struct Window {
    pub(crate) raw: Arc<WinitWindow>,
    pub(crate) event_loop: Option<EventLoop<()>>,
    should_quit: Arc<AtomicBool>,
}

impl Window {
    pub fn new(config: WindowConfig) -> Self {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let raw = WindowBuilder::new()
            .with_title(config.title)
            .with_inner_size(LogicalSize::new(config.width, config.height))
            .build(&event_loop)
            .expect("Failed to create window");

        Self {
            raw: Arc::new(raw),
            event_loop: Some(event_loop),
            should_quit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn take_event_loop(&mut self) -> EventLoop<()> {
        self.event_loop.take().expect("EventLoop already taken")
    }

    pub fn width(&self) -> u32 {
        self.raw.inner_size().width
    }

    pub fn height(&self) -> u32 {
        self.raw.inner_size().height
    }

    pub fn set_title(&self, title: &str) {
        self.raw.set_title(title);
    }

    pub fn set_fullscreen(&self, enabled: bool) {
        if enabled {
            self.raw.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            self.raw.set_fullscreen(None);
        }
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.raw.set_cursor_visible(visible);
    }

    pub fn set_cursor_grabbed(&self, grabbed: bool) {
        if grabbed {
            self.raw
                .set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_| self.raw.set_cursor_grab(CursorGrabMode::Locked))
                .unwrap_or_else(|e| log::warn!("Failed to grab cursor: {e}"));
            self.raw.set_cursor_visible(false);
        } else {
            self.raw
                .set_cursor_grab(CursorGrabMode::None)
                .unwrap_or_else(|e| log::warn!("Failed to release cursor: {e}"));
            self.raw.set_cursor_visible(true);
        }
    }

    pub fn quit(&self) {
        self.should_quit.store(true, Ordering::Relaxed);
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit.load(Ordering::Relaxed)
    }

    pub fn request_redraw(&self) {
        self.raw.request_redraw();
    }
}
