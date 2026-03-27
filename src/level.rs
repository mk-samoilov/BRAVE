use std::sync::Arc;

use brave_core::prelude::*;

pub fn setup_level(game: &mut Engine) {
    let cube_mesh = game.render().create_cube();

    game.world.spawn("box_1")
        .with(Transform::new(-2.1, -1.6, 1.1))
        .with(MeshRenderer::new(Arc::clone(&cube_mesh)));

    game.world.spawn("box_2")
        .with(Transform::new(0.11, 1.43, -1.65))
        .with(MeshRenderer::new(Arc::clone(&cube_mesh)));

    game.world.spawn("box_3")
        .with(Transform::new(2.6, 0.5, 0.0))
        .with(MeshRenderer::new(Arc::clone(&cube_mesh)));

    game.world.spawn("box_4")
        .with(Transform::new(2.21, 0.123, 4.12))
        .with(MeshRenderer::new(cube_mesh));
}
