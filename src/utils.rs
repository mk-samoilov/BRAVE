use brave_core::prelude::*;

use std::sync::Arc;

use brave_core::prelude::{MeshRenderer, Transform, Vec3};

pub fn place_gltf_actor(
    game: &mut Engine,
    model: &str,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    scale: f32,
) {
    let actor = game.assets_mut().load_gltf(&format!("{model}/scene.gltf"));

    let model_name = model
        .split_once("/")
        .map(|(name, _)| name)
        .unwrap_or(model);

    for (i, prim) in actor.primitives.iter().enumerate() {
        game.world.spawn(&format!("{}_{}", model_name, i))
            .with(Transform {
                scale: Vec3::splat(scale),
                ..Transform::new(pos_x, pos_y, pos_z)
            })
            .with(MeshRenderer {
                mesh:       Arc::clone(&prim.mesh),
                texture:    prim.texture.clone(),
                normal_map: prim.normal_map.clone(),
                orm_map:    prim.orm_map.clone(),
                base_color: prim.base_color,
                metallic:   prim.metallic,
                roughness:  prim.roughness,
            });
    }
}
