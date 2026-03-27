use std::sync::Arc;

use brave_core::prelude::*;

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

    let chair = game.assets_mut().load_gltf("classic_chair/scene.gltf");
    for (i, prim) in chair.primitives.iter().enumerate() {
        game.world.spawn(&format!("chair_{}", i))
            .with(Transform { scale: Vec3::splat(2.0), ..Transform::new(0.0, 0.0, 0.0) })
            .with(MeshRenderer {
                mesh:       Arc::clone(&prim.mesh),
                texture:    prim.texture.clone(),
                base_color: prim.base_color,
            });
    }
}
