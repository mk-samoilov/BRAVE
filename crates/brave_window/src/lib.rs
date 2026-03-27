use std::sync::Arc;
use winit::window::{Fullscreen, WindowAttributes};

pub use winit::event_loop::{ActiveEventLoop, EventLoop};
pub use winit::event::{WindowEvent, DeviceEvent, DeviceId};
pub use winit::window::WindowId;
pub use winit::application::ApplicationHandler;
pub use winit::dpi::PhysicalSize;

pub struct WindowConfig {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

pub struct Window {
    inner: Arc<winit::window::Window>,
    should_quit: bool,
}

impl Window {
    pub fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Self {
        let attrs = WindowAttributes::default()
            .with_title(config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height));

        let inner = Arc::new(
            event_loop.create_window(attrs).expect("Failed to create window"),
        );

        Self { inner, should_quit: false }
    }

    pub fn raw(&self) -> &Arc<winit::window::Window> {
        &self.inner
    }

    pub fn width(&self) -> u32 {
        self.inner.inner_size().width
    }

    pub fn height(&self) -> u32 {
        self.inner.inner_size().height
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn set_title(&self, title: &str) {
        self.inner.set_title(title);
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.inner.set_fullscreen(if fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.inner.set_cursor_visible(visible);
    }

    pub fn set_cursor_grabbed(&self, grabbed: bool) {
        use winit::window::CursorGrabMode;
        if grabbed {
            self.inner
                .set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_| self.inner.set_cursor_grab(CursorGrabMode::Locked))
                .ok();
        } else {
            self.inner.set_cursor_grab(CursorGrabMode::None).ok();
        }
    }

    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    pub fn window_id(&self) -> WindowId {
        self.inner.id()
    }
}
