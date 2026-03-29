fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // crates/brv_assets → crates → workspace root
    let workspace_root = std::path::PathBuf::from(&manifest_dir)
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf();

    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let key = read_asset_key(&cargo_toml_path);

    println!("cargo:rustc-env=BRAVE_COMPILED_ASSET_KEY={}", key);
    println!("cargo:rerun-if-changed={}", cargo_toml_path.display());
}

fn read_asset_key(cargo_toml_path: &std::path::Path) -> String {
    let src = std::fs::read_to_string(cargo_toml_path).unwrap_or_default();
    let doc: toml::Value = toml::from_str(&src).unwrap_or(toml::Value::Table(Default::default()));
    doc.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("brave"))
        .and_then(|b| b.get("asset_key"))
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string()
}
