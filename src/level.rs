use std::sync::Arc;

use brave_core::prelude::*;

pub fn setup_level(game: &mut Engine) {
    let ground = game.render().create_plane(20.0);
    game.world.spawn("ground")
        .with(Transform::new(0.0, 0.0, 0.0))
        .with(MeshRenderer::new(ground));

    let cube = game.render().create_cube();

    game.world.spawn("box_1")
        .with(Transform::new(3.0, 0.5, 3.0))
        .with(MeshRenderer::new(Arc::clone(&cube)));

    game.world.spawn("box_2")
        .with(Transform::new(-3.0, 0.5, 2.0))
        .with(MeshRenderer::new(Arc::clone(&cube)));

    game.world.spawn("box_3")
        .with(Transform::new(0.0, 0.5, 5.0))
        .with(MeshRenderer::new(cube));
}
