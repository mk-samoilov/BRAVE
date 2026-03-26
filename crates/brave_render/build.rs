use std::path::Path;
use std::process::Command;

fn main() {
    let shaders = [
        "../../assets/shaders/mesh.vert.glsl",
        "../../assets/shaders/mesh.frag.glsl",
        "../../assets/shaders/shadow.vert.glsl",
    ];

    let out_dir = std::env::var("OUT_DIR").unwrap();

    for shader in &shaders {
        println!("cargo:rerun-if-changed={}", shader);

        let file_name = Path::new(shader).file_name().unwrap().to_str().unwrap();
        let out_path = format!("{}/{}.spv", out_dir, file_name);

        let status = Command::new("glslangValidator")
            .args(["-V", shader, "-o", &out_path])
            .status()
            .expect("glslangValidator не найден. Установите: sudo apt install glslang-tools");

        assert!(
            status.success(),
            "Ошибка компиляции шейдера: {}",
            shader
        );
    }
}
