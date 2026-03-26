use std::path::Path;
use std::process::Command;

pub fn load(path: &Path) -> Vec<u32> {
    let tmp = std::env::temp_dir().join(format!(
        "brave_shader_{}.spv",
        path.file_name().unwrap().to_string_lossy()
    ));

    let status = Command::new("glslangValidator")
        .args(["-V", path.to_str().unwrap(), "-o", tmp.to_str().unwrap()])
        .status()
        .expect("glslangValidator not found: sudo apt install glslang-tools");

    assert!(status.success(), "Shader compilation failed: {}", path.display());

    let bytes = std::fs::read(&tmp)
        .unwrap_or_else(|e| panic!("Failed to read SPIR-V '{}': {}", tmp.display(), e));

    assert_eq!(bytes.len() % 4, 0, "SPIR-V size must be a multiple of 4");
    bytes.chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
