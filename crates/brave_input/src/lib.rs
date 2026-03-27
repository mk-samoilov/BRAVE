use std::collections::HashSet;
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub use winit::keyboard::KeyCode as Key;
pub use winit::event::MouseButton;

pub struct Input {
    pressed: HashSet<KeyCode>,
    held: HashSet<KeyCode>,
    released: HashSet<KeyCode>,

    mouse_pressed: HashSet<WinitMouseButton>,
    mouse_held: HashSet<WinitMouseButton>,
    mouse_released: HashSet<WinitMouseButton>,

    mouse_position: (f32, f32),
    mouse_delta: (f32, f32),
    mouse_delta_accum: (f32, f32),

    mouse_scroll: f32,
    scroll_accum: f32,
}

impl Input {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            held: HashSet::new(),
            released: HashSet::new(),
            mouse_pressed: HashSet::new(),
            mouse_held: HashSet::new(),
            mouse_released: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            mouse_delta_accum: (0.0, 0.0),
            mouse_scroll: 0.0,
            scroll_accum: 0.0,
        }
    }

    pub fn is_key_pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    pub fn is_key_held(&self, key: Key) -> bool {
        self.held.contains(&key)
    }

    pub fn is_key_released(&self, key: Key) -> bool {
        self.released.contains(&key)
    }

    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse_position
    }

    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    pub fn is_mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_held.contains(&button)
    }

    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_released.contains(&button)
    }

    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll
    }

    pub fn begin_frame(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.mouse_delta = self.mouse_delta_accum;
        self.mouse_delta_accum = (0.0, 0.0);
        self.mouse_scroll = self.scroll_accum;
        self.scroll_accum = 0.0;
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed if !event.repeat => {
                            self.pressed.insert(code);
                            self.held.insert(code);
                        }
                        ElementState::Released => {
                            self.released.insert(code);
                            self.held.remove(&code);
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    self.mouse_pressed.insert(*button);
                    self.mouse_held.insert(*button);
                }
                ElementState::Released => {
                    self.mouse_released.insert(*button);
                    self.mouse_held.remove(button);
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.1,
                };
                self.scroll_accum += y;
            }
            _ => {}
        }
    }

    pub fn handle_mouse_motion(&mut self, delta: (f64, f64)) {
        self.mouse_delta_accum.0 += delta.0 as f32;
        self.mouse_delta_accum.1 += delta.1 as f32;
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
