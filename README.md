# BRAVE

**B**lazing **R**ust **A**dvanced **V**ulkan **E**ngine - Minimal 3D engine in Rust with Vulkan

## Dependencies

```
sudo apt install glslang-tools vulkan-tools libvulkan-dev vulkan-validationlayers
```

## Build & Run

```bash
cargo run              # debug (Vulkan validation layers enabled)
cargo run --release    # release (.ast archives packed automatically)
cargo test             # tests
cargo clippy           # linter
```

## Structure

```
src/               - game
  crates/
    brave_core     - Engine, Plugin, game loop
    brave_ecs      - ECS (World, Entity, Component, Script)
    brave_render   - Vulkan renderer (forward, shadow maps)
    brave_assets   - asset loading (.glb, .png, .glsl / .ast in release)
    brave_scene    - transform hierarchy
    brave_input    - keyboard, mouse
    brave_window   - winit wrapper
    brave_math     - glam + Color
  assets/
    shaders/       - GLSL sources (compiled to SPIR-V at build time)
    models/        - .glb/.gltf
    textures/      - .png/.jpg/.hdr
```
