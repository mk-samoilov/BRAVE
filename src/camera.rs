use brv_core::prelude::*;
use std::cell::Cell;

thread_local! {
    static PITCH: Cell<f32> = const { Cell::new(0.0) };
    static YAW:   Cell<f32> = const { Cell::new(0.0) };
    static SPEED: Cell<f32> = const { Cell::new(6.0) };
    static VEL_X: Cell<f32> = const { Cell::new(0.0) };
    static VEL_Y: Cell<f32> = const { Cell::new(0.0) };
    static VEL_Z: Cell<f32> = const { Cell::new(0.0) };
}

pub fn update_system(game: &mut Engine) {
    let ptr = game.world.get_obj_mut("camera").map(|o| o as *mut Object);
    if let Some(ptr) = ptr {
        update(unsafe { &mut *ptr }, game);
    }
}

pub fn update(obj: &mut Object, game: &mut Engine) {
    let input = match game.input.as_ref() {
        Some(i) => i,
        None => return,
    };

    let scroll = input.mouse_scroll();
    if scroll != 0.0 {
        SPEED.with(|s| s.set((s.get() * 1.2f32.powf(scroll)).max(0.1)));
    }

    let rmb = input.is_mouse_held(MouseButton::Right);
    if let Some(window) = game.window.as_ref() {
        window.set_cursor_grabbed(rmb);
    }

    if rmb {
        let (dx, dy) = input.mouse_delta();
        let sensitivity = 0.00155;
        PITCH.with(|p| {
            let v = (p.get() + dy * sensitivity)
                .clamp(-(std::f32::consts::FRAC_PI_2 - 0.01), std::f32::consts::FRAC_PI_2 - 0.01);
            p.set(v);
        });
        YAW.with(|y| y.set(y.get() - dx * sensitivity));
    }

    let pitch = PITCH.with(|p| p.get());
    let yaw   = YAW.with(|y| y.get());
    obj.rotate.set_quat(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch));

    let speed = SPEED.with(|s| s.get());
    let quat    = obj.rotate.quat();
    let forward = quat * Vec3::Z;
    let right   = quat * Vec3::X;

    let mut target = Vec3::ZERO;
    if input.is_key_held(Key::W) { target += forward; }
    if input.is_key_held(Key::S) { target -= forward; }
    if input.is_key_held(Key::A) { target += right; }
    if input.is_key_held(Key::D) { target -= right; }
    if input.is_key_held(Key::Space)  { target += Vec3::Y; }
    if input.is_key_held(Key::LShift) { target -= Vec3::Y; }

    if target.length_squared() > 0.0 {
        target = target.normalize() * speed;
    }

    let dt = game.time.delta();
    let smoothing = 1.0 - (-12.0 * dt).exp();

    VEL_X.with(|v| v.set(v.get() + (target.x - v.get()) * smoothing));
    VEL_Y.with(|v| v.set(v.get() + (target.y - v.get()) * smoothing));
    VEL_Z.with(|v| v.set(v.get() + (target.z - v.get()) * smoothing));

    let vel = Vec3::new(
        VEL_X.with(|v| v.get()),
        VEL_Y.with(|v| v.get()),
        VEL_Z.with(|v| v.get()),
    );

    let pos = obj.transform.get() + vel * dt;
    obj.transform.set(pos.x, pos.y, pos.z);
}
