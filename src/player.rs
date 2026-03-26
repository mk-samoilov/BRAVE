use brave_core::prelude::*;

pub struct FlyCamera {
    pub yaw:   f32,
    pub pitch: f32,
    pub speed: f32,
}

impl Component for FlyCamera {}

pub fn player_look(game: &mut Engine) {
    if game.input().is_mouse_pressed(MouseButton::Right) {
        game.window_mut().set_cursor_grabbed(true);
        game.window_mut().set_cursor_visible(false);
    }
    if game.input().is_mouse_released(MouseButton::Right) {
        game.window_mut().set_cursor_grabbed(false);
        game.window_mut().set_cursor_visible(true);
    }

    let looking = game.input().is_mouse_held(MouseButton::Right);
    let scroll  = game.input().mouse_scroll();
    let (dx, dy) = game.input().mouse_delta();

    let camera = game.world.get_mut("camera");

    {
        let fc = camera.get_mut::<FlyCamera>();
        fc.speed = (fc.speed + scroll * 1.0).clamp(0.5, 100.0);
        if looking {
            fc.yaw   -= dx * 0.004;
            fc.pitch  = (fc.pitch - dy * 0.004).clamp(-1.55, 1.55);
        }
    }

    let (yaw, pitch) = {
        let fc = camera.get::<FlyCamera>();
        (fc.yaw, fc.pitch)
    };

    camera.get_mut::<Transform>().rotation =
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
}

pub fn player_update(entity: &mut Entity, game: &mut Engine) {
    let dt   = game.time.fixed_delta();
    let w    = game.input().is_key_held(Key::KeyW);
    let s    = game.input().is_key_held(Key::KeyS);
    let a    = game.input().is_key_held(Key::KeyA);
    let d    = game.input().is_key_held(Key::KeyD);
    let up   = game.input().is_key_held(Key::Space);
    let down = game.input().is_key_held(Key::ShiftLeft)
            || game.input().is_key_held(Key::ShiftRight);

    let (yaw, pitch, speed) = {
        let fc = entity.get::<FlyCamera>();
        (fc.yaw, fc.pitch, fc.speed)
    };

    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    let forward  = rotation * (-Vec3::Z);
    let right    = rotation * Vec3::X;
    let mv       = speed * dt;

    let tr = entity.get_mut::<Transform>();
    if w    { tr.position += forward * mv; }
    if s    { tr.position -= forward * mv; }
    if a    { tr.position -= right   * mv; }
    if d    { tr.position += right   * mv; }
    if up   { tr.position.y += mv; }
    if down { tr.position.y -= mv; }
}
