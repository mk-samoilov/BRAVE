use brave_core::prelude::*;

use crate::level::setup_level;
use crate::player::player_update;

pub fn setup(game: &mut Engine) {
    game.world.spawn("camera")
        .with(Transform::new(0.0, 3.0, 8.0))
        .with(Camera { fov: 60.0, near: 0.1, far: 1000.0 });

    game.world.spawn("sun")
        .with(Transform::new(5.0, 10.0, -5.0))
        .with(DirectionalLight { color: Color::WHITE, intensity: 1.0, shadows: true });

    game.world.spawn("ambient")
        .with(AmbientLight { color: Color::WHITE, intensity: 0.2 });

    let player_mesh = game.render().create_cube();
    game.world.spawn("player")
        .with(Transform::new(0.0, 0.5, 0.0))
        .with(MeshRenderer::new(player_mesh))
        .with(Script::new(player_update));

    setup_level(game);
}

pub fn update(game: &mut Engine) {
    if game.input().is_key_pressed(Key::Escape) {
        game.window_mut().quit();
    }
}
