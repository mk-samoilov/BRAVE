# Структура проекта

## Дерево файлов

```
brave/
├── Cargo.toml                       # workspace + [[bin]]
│
├── src/                            # игра
│   ├── main.rs
│   └── game.rs
│
├── crates/                         # движок BRAVE
│   ├── brv_core/                   # engine, plugin, game loop
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs           # Engine struct
│   │       ├── plugin.rs           # Plugin trait, TypeId защита
│   │       └── time.rs             # Time struct, set_fps, set_physics_rate
│   │
│   ├── brv_ecs/                    # entity component system (свой с нуля)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # реэкспорт
│   │       ├── world.rs            # World: spawn, get_obj, remove_obj
│   │       ├── object.rs           # Object struct, встроенные поля
│   │       ├── transform.rs        # TransformField: set/get
│   │       ├── rotate.rs           # RotateField: set/get/look_at_obj/look_at_vec
│   │       ├── visible.rs          # VisibleField: set/get
│   │       ├── component.rs        # Component trait, Option-поля (mesh, camera, light)
│   │       ├── script.rs           # Script component
│   │       └── system.rs           # система вызова систем и скриптов
│   │
│   ├── brv_render/                 # Vulkan рендерер
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs          # VkInstance, Device, Queues
│   │       ├── swapchain.rs        # Swapchain + recreation
│   │       ├── pipeline.rs         # Graphics pipeline, PSO cache
│   │       ├── buffer.rs           # Vertex, index, uniform buffers
│   │       ├── texture.rs          # Images, samplers
│   │       ├── descriptor.rs       # Descriptor sets/layouts
│   │       ├── mesh.rs             # Mesh struct, draw commands
│   │       ├── material.rs         # PBR material (metallic/roughness)
│   │       ├── camera.rs           # Camera component, view/projection
│   │       └── light.rs            # 4 типа света, shadow maps
│   │
│   ├── brv_assets/                 # загрузка и упаковка ресурсов
│   │   ├── Cargo.toml
│   │   ├── build.rs                # компиляция assets → .ast (только release)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs          # AssetManager, LRU кеш
│   │       ├── loader.rs           # загрузка из .ast (release) или файлов (debug)
│   │       ├── gltf_loader.rs      # GLTF папка → vertex buffers + RGBA
│   │       ├── glb_loader.rs       # GLB файл → vertex buffers + RGBA
│   │       ├── image_loader.rs     # PNG/JPG/HDR → RGBA
│   │       ├── shader_loader.rs    # GLSL → SPIR-V
│   │       ├── packer.rs           # упаковщик в .ast файлы
│   │       └── crypto.rs           # XOR шифрование astdb.lock
│   │
│   ├── brv_scene/                  # сцена, трансформы, rotation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── transform.rs        # Transform данные
│   │       ├── hierarchy.rs        # Parent-child, dirty flags
│   │       └── scene.rs            # Scene утилиты
│   │
│   ├── brv_input/                  # клавиатура, мышь
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs              # Input struct, polling API
│   │
│   ├── brv_window/                 # winit обёртка
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs              # Window struct, WindowPlugin
│   │
│   ├── brv_math/                   # Vec3, Mat4, Quat (glam)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs              # реэкспорт glam типов
│   │
│   └── brv_colors/                 # Color struct + пресеты
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs              # Color, пресеты (WHITE, WARM, etc.)
│
└── assets/
    ├── shaders/
    │   ├── mesh.vert.glsl
    │   ├── mesh.frag.glsl
    │   └── shadow.vert.glsl
    ├── models/
    │   ├── chair/                  # GLTF модель (папка)
    │   │   ├── scene.gltf
    │   │   └── textures/
    │   │       └── diffuse.png
    │   └── weapon.glb              # GLB модель (один файл)
    └── textures/
        ├── grass.png
        └── normal.png
```

## Корневой Cargo.toml

```toml
[workspace]
members = ["crates/*"]

[package]
name = "my-game"
edition = "2024"

[dependencies]
brv_core   = { path = "crates/brv_core" }
brv_ecs    = { path = "crates/brv_ecs" }
brv_render = { path = "crates/brv_render" }
brv_assets = { path = "crates/brv_assets" }
brv_scene  = { path = "crates/brv_scene" }
brv_input  = { path = "crates/brv_input" }
brv_window = { path = "crates/brv_window" }
brv_math   = { path = "crates/brv_math" }
brv_colors = { path = "crates/brv_colors" }
```

Это workspace: каждый крейт в `crates/` компилируется отдельно, корневой пакет — бинарный проект (игра). Такая структура общепринята в Rust (Bevy, Fyrox, Veloren используют аналогичный подход).
