use brave_window::{
    ActiveEventLoop, ApplicationHandler, DeviceEvent, DeviceId,
    EventLoop, WindowEvent, WindowId,
};
use crate::engine::Engine;

pub struct AppRunner {
    pub engine: Engine,
}

impl AppRunner {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }
}

impl ApplicationHandler for AppRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.engine.create_window_if_pending(event_loop);
        self.engine.create_renderer_if_pending();

        if !self.engine.startup_done {
            self.engine.run_startup();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(input) = &mut self.engine.input {
            input.handle_window_event(&event);
        }

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.engine.run_frame();

                if self.engine.window.as_ref().is_some_and(|w| w.should_quit()) {
                    event_loop.exit();
                    return;
                }

                if let Some(window) = &self.engine.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(_) => {}
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && let Some(input) = &mut self.engine.input
        {
            input.handle_mouse_motion(delta);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.engine.window {
            window.request_redraw();
        }
    }
}

pub fn run_brave(engine: Engine) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut runner = AppRunner::new(engine);
    event_loop.run_app(&mut runner).expect("Event loop error");
}
