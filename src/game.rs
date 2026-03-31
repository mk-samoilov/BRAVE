use brv_core::prelude::*;

pub fn setup(game: &mut Engine) {
    game.window.as_ref().unwrap().set_type(WindowType::Fullscreen);

    let cam = game.world.spawn("camera");
    cam.transform.set(0.0, 3.0, -5.0);
    cam.camera.set(Camera { fov: 60.0, near: 0.1, far: 1000.0 });
    game.add_system(crate::camera::update_system);

    let cube_mesh = make_cube();
    let obj = game.world.spawn("cube");
    obj.transform.set(0.0, 0.0, 0.0);
    obj.mesh.set(cube_mesh);

    let table_mesh: MeshComponent = game.assets.as_mut().unwrap()
        .load("models/classic_table", AssetType::GLTFModel)
        .into();
    let table = game.world.spawn("table");
    table.transform.set(2.5, 0.0, 0.0);
    table.mesh.set(table_mesh);

    let chair_mesh: MeshComponent = game.assets.as_mut().unwrap()
        .load("models/classic_chair", AssetType::GLTFModel)
        .into();
    let chair = game.world.spawn("chair");
    chair.transform.set(2.3, 0.0, 0.45);
    chair.transform.set_scale(0.185, 0.185, 0.185);
    chair.rotate.set_quat(
        Quat::from_rotation_y(std::f32::consts::PI + 0.25)
        * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    );
    chair.mesh.set(chair_mesh);
}

pub fn update(game: &mut Engine) {
    if let Some(input) = &game.input {
        if input.is_key_pressed(Key::Escape) {
            game.window.as_ref().unwrap().quit();
        }
    }
}

fn make_cube() -> MeshComponent {
    let positions: &[[f32; 3]] = &[
        [-0.5, -0.5,  0.5], [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [-0.5,  0.5,  0.5],
        [-0.5, -0.5, -0.5], [-0.5,  0.5, -0.5], [ 0.5,  0.5, -0.5], [ 0.5, -0.5, -0.5],
        [-0.5,  0.5, -0.5], [-0.5,  0.5,  0.5], [ 0.5,  0.5,  0.5], [ 0.5,  0.5, -0.5],
        [-0.5, -0.5, -0.5], [ 0.5, -0.5, -0.5], [ 0.5, -0.5,  0.5], [-0.5, -0.5,  0.5],
        [ 0.5, -0.5, -0.5], [ 0.5,  0.5, -0.5], [ 0.5,  0.5,  0.5], [ 0.5, -0.5,  0.5],
        [-0.5, -0.5, -0.5], [-0.5, -0.5,  0.5], [-0.5,  0.5,  0.5], [-0.5,  0.5, -0.5],
    ];
    let normals: &[[f32; 3]] = &[
        [ 0.0,  0.0,  1.0], [ 0.0,  0.0,  1.0], [ 0.0,  0.0,  1.0], [ 0.0,  0.0,  1.0],
        [ 0.0,  0.0, -1.0], [ 0.0,  0.0, -1.0], [ 0.0,  0.0, -1.0], [ 0.0,  0.0, -1.0],
        [ 0.0,  1.0,  0.0], [ 0.0,  1.0,  0.0], [ 0.0,  1.0,  0.0], [ 0.0,  1.0,  0.0],
        [ 0.0, -1.0,  0.0], [ 0.0, -1.0,  0.0], [ 0.0, -1.0,  0.0], [ 0.0, -1.0,  0.0],
        [ 1.0,  0.0,  0.0], [ 1.0,  0.0,  0.0], [ 1.0,  0.0,  0.0], [ 1.0,  0.0,  0.0],
        [-1.0,  0.0,  0.0], [-1.0,  0.0,  0.0], [-1.0,  0.0,  0.0], [-1.0,  0.0,  0.0],
    ];

    let vertices: Vec<Vertex> = (0..24)
        .map(|i| Vertex { position: positions[i], normal: normals[i], uv: [0.0, 0.0] })
        .collect();

    let indices: Vec<u32> = (0..6u32)
        .flat_map(|face| {
            let b = face * 4;
            [b, b + 1, b + 2, b, b + 2, b + 3]
        })
        .collect();

    MeshComponent::new(vertices, indices)
}
