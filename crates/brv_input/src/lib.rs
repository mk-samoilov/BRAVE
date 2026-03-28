use std::collections::HashSet;
use winit::{
    event::{DeviceEvent, ElementState, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

/// Физические клавиши клавиатуры.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Буквы
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    // Цифры (основной ряд)
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    // Служебные
    Space, Escape, Enter, Tab, Backspace, Delete,
    // F-клавиши
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    // Модификаторы
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt,
    // Стрелки
    Up, Down, Left, Right,
}

impl Key {
    fn from_keycode(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::KeyA => Some(Key::A),
            KeyCode::KeyB => Some(Key::B),
            KeyCode::KeyC => Some(Key::C),
            KeyCode::KeyD => Some(Key::D),
            KeyCode::KeyE => Some(Key::E),
            KeyCode::KeyF => Some(Key::F),
            KeyCode::KeyG => Some(Key::G),
            KeyCode::KeyH => Some(Key::H),
            KeyCode::KeyI => Some(Key::I),
            KeyCode::KeyJ => Some(Key::J),
            KeyCode::KeyK => Some(Key::K),
            KeyCode::KeyL => Some(Key::L),
            KeyCode::KeyM => Some(Key::M),
            KeyCode::KeyN => Some(Key::N),
            KeyCode::KeyO => Some(Key::O),
            KeyCode::KeyP => Some(Key::P),
            KeyCode::KeyQ => Some(Key::Q),
            KeyCode::KeyR => Some(Key::R),
            KeyCode::KeyS => Some(Key::S),
            KeyCode::KeyT => Some(Key::T),
            KeyCode::KeyU => Some(Key::U),
            KeyCode::KeyV => Some(Key::V),
            KeyCode::KeyW => Some(Key::W),
            KeyCode::KeyX => Some(Key::X),
            KeyCode::KeyY => Some(Key::Y),
            KeyCode::KeyZ => Some(Key::Z),
            KeyCode::Digit0 => Some(Key::Num0),
            KeyCode::Digit1 => Some(Key::Num1),
            KeyCode::Digit2 => Some(Key::Num2),
            KeyCode::Digit3 => Some(Key::Num3),
            KeyCode::Digit4 => Some(Key::Num4),
            KeyCode::Digit5 => Some(Key::Num5),
            KeyCode::Digit6 => Some(Key::Num6),
            KeyCode::Digit7 => Some(Key::Num7),
            KeyCode::Digit8 => Some(Key::Num8),
            KeyCode::Digit9 => Some(Key::Num9),
            KeyCode::Space => Some(Key::Space),
            KeyCode::Escape => Some(Key::Escape),
            KeyCode::Enter => Some(Key::Enter),
            KeyCode::Tab => Some(Key::Tab),
            KeyCode::Backspace => Some(Key::Backspace),
            KeyCode::Delete => Some(Key::Delete),
            KeyCode::F1 => Some(Key::F1),
            KeyCode::F2 => Some(Key::F2),
            KeyCode::F3 => Some(Key::F3),
            KeyCode::F4 => Some(Key::F4),
            KeyCode::F5 => Some(Key::F5),
            KeyCode::F6 => Some(Key::F6),
            KeyCode::F7 => Some(Key::F7),
            KeyCode::F8 => Some(Key::F8),
            KeyCode::F9 => Some(Key::F9),
            KeyCode::F10 => Some(Key::F10),
            KeyCode::F11 => Some(Key::F11),
            KeyCode::F12 => Some(Key::F12),
            KeyCode::ShiftLeft => Some(Key::LShift),
            KeyCode::ShiftRight => Some(Key::RShift),
            KeyCode::ControlLeft => Some(Key::LCtrl),
            KeyCode::ControlRight => Some(Key::RCtrl),
            KeyCode::AltLeft => Some(Key::LAlt),
            KeyCode::AltRight => Some(Key::RAlt),
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            KeyCode::ArrowLeft => Some(Key::Left),
            KeyCode::ArrowRight => Some(Key::Right),
            _ => None,
        }
    }
}

/// Кнопки мыши.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    fn from_winit(btn: WinitMouseButton) -> Option<Self> {
        match btn {
            WinitMouseButton::Left => Some(MouseButton::Left),
            WinitMouseButton::Right => Some(MouseButton::Right),
            WinitMouseButton::Middle => Some(MouseButton::Middle),
            _ => None,
        }
    }
}

pub struct Input {
    // Клавиатура — два набора для определения pressed/released
    current_keys: HashSet<Key>,
    previous_keys: HashSet<Key>,

    // Мышь
    mouse_pos: (f32, f32),
    mouse_delta_accum: (f32, f32), // накапливается из DeviceEvent за кадр
    mouse_delta: (f32, f32),       // финальное значение за кадр
    mouse_scroll_accum: f32,
    mouse_scroll: f32,

    // Кнопки мыши
    current_mouse: HashSet<MouseButton>,
    previous_mouse: HashSet<MouseButton>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            current_keys: HashSet::new(),
            previous_keys: HashSet::new(),
            mouse_pos: (0.0, 0.0),
            mouse_delta_accum: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            mouse_scroll_accum: 0.0,
            mouse_scroll: 0.0,
            current_mouse: HashSet::new(),
            previous_mouse: HashSet::new(),
        }
    }

    // ── Вызывается game loop'ом в начале кадра ──────────────────────────────

    /// Сбрасывает previous-состояние и переносит delta/scroll в финальные поля.
    /// Вызывать ПЕРЕД обработкой событий нового кадра.
    pub fn begin_frame(&mut self) {
        self.previous_keys = self.current_keys.clone();
        self.previous_mouse = self.current_mouse.clone();
        self.mouse_delta = self.mouse_delta_accum;
        self.mouse_delta_accum = (0.0, 0.0);
        self.mouse_scroll = self.mouse_scroll_accum;
        self.mouse_scroll_accum = 0.0;
    }

    // ── Обработка winit событий ─────────────────────────────────────────────

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    if let Some(key) = Key::from_keycode(keycode) {
                        match event.state {
                            ElementState::Pressed => { self.current_keys.insert(key); }
                            ElementState::Released => { self.current_keys.remove(&key); }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                self.mouse_scroll_accum += scroll;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(btn) = MouseButton::from_winit(*button) {
                    match state {
                        ElementState::Pressed => { self.current_mouse.insert(btn); }
                        ElementState::Released => { self.current_mouse.remove(&btn); }
                    }
                }
            }
            _ => {}
        }
    }

    /// DeviceEvent::MouseMotion даёт относительное движение (работает при захваченном курсоре).
    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.mouse_delta_accum.0 += delta.0 as f32;
            self.mouse_delta_accum.1 += delta.1 as f32;
        }
    }

    // ── Polling API ──────────────────────────────────────────────────────────

    /// Клавиша нажата в этом кадре (true только один кадр).
    pub fn is_key_pressed(&self, key: Key) -> bool {
        self.current_keys.contains(&key) && !self.previous_keys.contains(&key)
    }

    /// Клавиша зажата (true пока держишь).
    pub fn is_key_held(&self, key: Key) -> bool {
        self.current_keys.contains(&key)
    }

    /// Клавиша отпущена в этом кадре (true только один кадр).
    pub fn is_key_released(&self, key: Key) -> bool {
        !self.current_keys.contains(&key) && self.previous_keys.contains(&key)
    }

    /// Абсолютная позиция курсора в пикселях.
    pub fn mouse_position(&self) -> (f32, f32) {
        self.mouse_pos
    }

    /// Относительное движение мыши за кадр (dx, dy).
    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    /// Прокрутка колёсика за кадр: >0 вверх, <0 вниз, 0.0 не крутили.
    pub fn mouse_scroll(&self) -> f32 {
        self.mouse_scroll
    }

    /// Кнопка мыши нажата в этом кадре.
    pub fn is_mouse_pressed(&self, btn: MouseButton) -> bool {
        self.current_mouse.contains(&btn) && !self.previous_mouse.contains(&btn)
    }

    /// Кнопка мыши зажата.
    pub fn is_mouse_held(&self, btn: MouseButton) -> bool {
        self.current_mouse.contains(&btn)
    }

    /// Кнопка мыши отпущена в этом кадре.
    pub fn is_mouse_released(&self, btn: MouseButton) -> bool {
        !self.current_mouse.contains(&btn) && self.previous_mouse.contains(&btn)
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
