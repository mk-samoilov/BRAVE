use std::sync::Arc;

use brave_core::prelude::*;

use crate::utils::place_gltf_actor;

pub fn setup_level(game: &mut Engine) {
    let cube_mesh = shapes::cube(game.render().ctx(), game.render().command_pool(), 8);
    let sphere_mesh = shapes::sphere(game.render().ctx(), game.render().command_pool(), 64);

    game.world.spawn("box_1")
        .with(Transform::new(-2.1, -1.6, 1.1))
        .with(MeshRenderer::new(Arc::clone(&cube_mesh)));

    game.world.spawn("sphere_1")
        .with(Transform::new(0.11, 1.43, -1.65))
        .with(MeshRenderer::new(Arc::clone(&sphere_mesh)));

    game.world.spawn("box_2")
        .with(Transform::new(2.6, 0.5, 0.0))
        .with(MeshRenderer::new(Arc::clone(&cube_mesh)));

    game.world.spawn("box_3")
        .with(Transform::new(2.21, 0.123, 4.12))
        .with(MeshRenderer::new(cube_mesh));

    place_gltf_actor(game, "classic_chair", 1.0, 0.0, 1.6, 2.2);

    place_gltf_actor(game, "classic_table", 2.2, 2.0, 2.5, 0.0335);
    place_gltf_actor(game, "coffee_cup", 1.12, 2.1695, 3.41, 2.9);

    place_gltf_actor(game, "stand_mirror", 0.9, 1.94, 6.5, 0.432);

    place_gltf_actor(game, "beretta_92fs", 1.12, 3.2, 3.6, 0.001);
    place_gltf_actor(game, "glock_17", 1.12, 3.2, 3.5, 1.0)
}
