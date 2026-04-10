use brv_core::prelude::*;

use crate::base_meshes::make_cube;

pub fn setup(game: &mut Engine) {
    game.window.as_ref().unwrap().set_type(WindowType::Fullscreen);

    let cam = game.world.spawn("camera");

    cam.transform.set_pos(0.0, 3.0, -5.0);
    cam.camera.set(Camera { fov: 60.0, near: 0.1, far: 1000.0 });
    game.add_system(crate::camera::update_system);

    let sun = game.world.spawn("sun");

    sun.transform.set_pos(45.0, 35.0, 25.0);
    sun.rotate.set(-0.5, std::f32::consts::PI, 0.0);
    sun.light.set(DirectionalLight { color: Color::DAYLIGHT, intensity: 3.0 });

    let ambient = game.world.spawn("ambient");

    ambient.light.set(AmbientLight { color: Color::COOL, intensity: 0.1 });

    let cube = game.world.spawn("cube");

    cube.transform.set_pos(2.0, 2.02, 2.82);
    cube.mesh.set(make_cube());

    let table_mesh: MeshComponent = game.assets.as_mut().unwrap()
        .load("models/classic_table", AssetType::GLTFModel)
        .into();

    let table = game.world.spawn("table");

    table.transform.set_pos(2.3, 1.02, -1.82);
    table.transform.set_scale(0.017, 0.017, 0.017);
    table.mesh.set(table_mesh);

    let chair_mesh: MeshComponent = game.assets.as_mut().unwrap()
        .load("models/classic_chair", AssetType::GLTFModel)
        .into();

    let chair = game.world.spawn("chair");

    chair.transform.set_pos(2.3, 0.0, 0.45);
    chair.rotate.set_quat(Quat::from_rotation_y(std::f32::consts::PI + 0.25));
    chair.mesh.set(chair_mesh);
}

pub fn update(game: &mut Engine) {
    if let Some(input) = &game.input {
        if input.is_key_pressed(Key::Escape) {
            game.window.as_ref().unwrap().quit();
        }
    }
}
