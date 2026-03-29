pub fn load(full_path: &str) -> Vec<u32> {
    let src = std::fs::read_to_string(full_path)
        .unwrap_or_else(|e| panic!("Failed to read shader {}: {}", full_path, e));

    let stage = if full_path.contains(".vert") {
        naga::ShaderStage::Vertex
    } else if full_path.contains(".frag") {
        naga::ShaderStage::Fragment
    } else {
        panic!("Unknown shader stage for: {}", full_path)
    };

    let mut frontend = naga::front::glsl::Frontend::default();
    let module = frontend
        .parse(&naga::front::glsl::Options::from(stage), &src)
        .unwrap_or_else(|e| panic!("GLSL parse error in {}: {:?}", full_path, e));

    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::empty(),
        naga::valid::Capabilities::PUSH_CONSTANT,
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("Shader validation error in {}: {:?}", full_path, e));

    naga::back::spv::write_vec(
        &module,
        &info,
        &naga::back::spv::Options { lang_version: (1, 0), ..Default::default() },
        None,
    )
    .unwrap_or_else(|e| panic!("SPIR-V generation error in {}: {:?}", full_path, e))
}
