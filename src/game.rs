use brave_core::prelude::*;

use crate::level::setup_level;
use crate::player::{FlyCamera, player_update};

pub fn setup(game: &mut Engine) {
    game.world.spawn("camera")
        .with(Transform::new(0.0, 3.0, 8.0))
        .with(Camera { fov: 60.0, near: 0.1, far: 1000.0 })
        .with(FlyCamera { yaw: 0.0, pitch: 0.0, speed: 5.0 })
        .with(Script::new(player_update));

    game.world.spawn("sun")
        .with(Transform::new(5.9, 9.4, -17.2))
        .with(DirectionalLight { color: Color::WHITE, intensity: 5.2, shadows: true });

    let skybox = game.assets_mut().load_gltf("rooftop_day_skybox/scene.gltf");
    if let Some(prim) = skybox.primitives.first() {
        if let Some(tex) = &prim.texture {
            game.render_mut().set_skybox(std::sync::Arc::clone(tex));
            game.render_mut().set_skybox_blur(1.0);
        }
    }

    setup_level(game);
}

pub fn update(game: &mut Engine) {
    if game.input().is_key_pressed(Key::Escape) {
        game.window_mut().quit();
    }
}
