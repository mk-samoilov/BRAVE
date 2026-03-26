# BRAVE — Blazing Rust Advanced Vulkan Engine

## Спецификация движка (промпт для разработки)

---

## Общее описание

BRAVE — это мини 3D движок на Rust с рендерингом через Vulkan. Движок построен как workspace из крейтов, где `src/` — игра (бинарный проект), а `crates/` — модули движка.

Целевые платформы: Windows и Linux (Kubuntu).

---

## Важное правило разработки

> Если при реализации какой-либо части движка что-то непонятно или вызывает сомнения — **не спешить реализовывать**. Сначала спросить и уточнить. Это касается как текущей реализации, так и внедрения новых фич в будущем. При реализации текущего промпта — просто реализовывать как описано ниже, без самодеятельности.

---

## Структура проекта

```
brave/
├── Cargo.toml                       # workspace + [[bin]]
│
├── src/                            # игра
│   ├── main.rs
│   ├── game.rs
│   ├── player.rs
│   └── level.rs
│
├── crates/                         # движок BRAVE
│   ├── brave_core/                 # app, plugin, game loop
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs
│   │       ├── plugin.rs
│   │       └── time.rs
│   │
│   ├── brave_ecs/                  # entity component system (свой)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── world.rs
│   │       ├── component.rs
│   │       ├── system.rs
│   │       └── query.rs
│   │
│   ├── brave_render/               # Vulkan рендерер
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs
│   │       ├── swapchain.rs
│   │       ├── pipeline.rs
│   │       ├── buffer.rs
│   │       ├── texture.rs
│   │       ├── descriptor.rs
│   │       ├── mesh.rs
│   │       ├── material.rs
│   │       ├── camera.rs
│   │       └── light.rs
│   │
│   ├── brave_assets/               # загрузка ресурсов
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs
│   │       ├── handle.rs
│   │       ├── gltf_loader.rs
│   │       ├── image_loader.rs
│   │       └── shader_loader.rs
│   │
│   ├── brave_scene/                # сцена, трансформы
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── transform.rs
│   │       ├── hierarchy.rs
│   │       └── scene.rs
│   │
│   ├── brave_input/                # клавиатура, мышь
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── brave_window/               # winit обёртка
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── brave_math/                 # Vec3, Mat4, Quat (glam)
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
│
└── assets/
    ├── shaders/
    │   ├── mesh.vert.glsl
    │   ├── mesh.frag.glsl
    │   └── shadow.vert.glsl
    ├── models/
    │   ├── character.glb
    │   └── world.glb
    ├── textures/
    │   ├── albedo.png
    │   └── normal.png
    └── audio/
        └── bgm.ogg
```

Корневой `Cargo.toml`:

```toml
[workspace]
members = ["crates/*"]

[package]
name = "my-game"
edition = "2024"

[dependencies]
brave_core   = { path = "crates/brave_core" }
brave_ecs    = { path = "crates/brave_ecs" }
brave_render = { path = "crates/brave_render" }
brave_assets = { path = "crates/brave_assets" }
brave_scene  = { path = "crates/brave_scene" }
brave_input  = { path = "crates/brave_input" }
brave_window = { path = "crates/brave_window" }
brave_math   = { path = "crates/brave_math" }
```

---

## Модуль brave_core

### Engine

`Engine` — центральный объект движка. Один тип везде: и при создании в `main()`, и в системах как параметр `game`.

```rust
pub struct Engine {
    pub world: World,          // ECS
    pub assets: AssetManager,  // загрузка ассетов
    pub input: Input,          // клавиатура, мышь
    pub render: Renderer,      // Vulkan рендерер
    pub time: Time,            // delta time
    pub window: Window,        // окно
    pub ui: HudManager,        // ImGui HUD
    pub physics: Physics,      // AABB, raycast (позже)
}
```

Методы верхнего уровня:

```rust
Engine::new()                          // создать движок
game.add_plugin(plugin)                // подключить плагин
game.add_startup_system(fn)            // система один раз при старте
game.add_system(fn)                    // система каждый кадр
game.remove_system(fn)                 // убрать систему
game.run()                             // запустить game loop
```

### Plugin system

Каждый крейт движка экспортирует свой плагин. Трейт:

```rust
pub trait Plugin: 'static {
    fn build(&self, game: &mut Engine);
}
```

Пример плагина (brave_render):

```rust
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, game: &mut Engine) {
        let context = VulkanContext::new(&game.window);
        game.render = Renderer::new(context);
        game.add_system(render_system);
    }
}
```

Защита от двойной регистрации через `TypeId`. Порядок `add_plugin` определяет порядок инициализации.

### Game loop

Двойной цикл:
- **Fixed timestep** (60 тиков/сек) — логика, Script-функции на сущностях
- **Variable delta time** — рендер, обычные системы (add_system)

Классический паттерн с аккумулятором:

```
loop {
    let dt = вычислить_delta_time();
    accumulator += dt;

    // fixed update (60fps)
    while accumulator >= FIXED_DT {
        запустить_скрипты_сущностей();
        accumulator -= FIXED_DT;
    }

    // variable update (каждый кадр)
    запустить_системы(dt);
    рендер();
}
```

### Системы

Два типа регистрации:
- `add_startup_system(fn)` — вызывается один раз при старте, до основного цикла
- `add_system(fn)` — вызывается каждый кадр в variable update

Сигнатура:

```rust
fn my_system(game: &mut Engine) { ... }
```

Системы можно добавлять и удалять в рантайме из других систем:

```rust
fn some_system(game: &mut Engine) {
    if game.input.is_key_pressed(Key::Tab) {
        game.add_system(debug_overlay);
    }
}
```

### Time

```rust
pub struct Time {
    delta: f32,          // переменный dt текущего кадра
    fixed_delta: f32,    // фиксированный dt (1/60)
    elapsed: f64,        // время с запуска
}

game.time.delta()        // переменный dt
game.time.fixed_delta()  // 1/60
game.time.elapsed()      // секунды с запуска
```

---

## Модуль brave_ecs

Свой ECS с нуля. Основные типы:

### World

```rust
pub struct World { ... }

game.world.spawn("player")              // создать сущность с именем
    .with(Transform::new(0.0, 1.0, 0.0))
    .with(MeshRenderer::new(mesh))
    .with(Script::new(player_update));   // опциональный тик-скрипт

game.world.despawn("player")            // удалить сущность по имени
game.world.get("player")                // получить сущность по имени
```

### Entity

```rust
entity.get::<Transform>()              // получить компонент (чтение)
entity.get_mut::<Transform>()          // получить компонент (запись)
entity.has::<MeshRenderer>()           // проверить наличие
entity.add(component)                  // добавить компонент
entity.remove::<MeshRenderer>()        // удалить компонент
```

### Component

Трейт для пользовательских компонентов:

```rust
pub trait Component: 'static { }
```

Встроенные компоненты: `Transform`, `MeshRenderer`, `Camera`, `DirectionalLight`, `PointLight`, `SpotLight`, `AmbientLight`, `Script`.

### Script (тик-функция на сущности)

Опциональный компонент. Функция вызывается каждый fixed tick (60fps):

```rust
fn player_update(entity: &mut Entity, game: &Engine) {
    let transform = entity.get_mut::<Transform>();
    let speed = 5.0 * game.time.fixed_delta();

    if game.input.is_key_held(Key::W) {
        transform.position.z += speed;
    }
}

// привязка:
game.world.spawn("player")
    .with(Transform::new(0.0, 1.0, 0.0))
    .with(Script::new(player_update));  // не обязательно
```

Script — необязательный. Сущность работает и без него.

### Query (будущее)

```rust
// запрос сущностей с определёнными компонентами
for (transform, mesh) in game.world.query::<(&Transform, &MeshRenderer)>() {
    // ...
}
```

---

## Модуль brave_render

### Общее

- Vulkan через крейт `ash`
- Forward rendering
- Одно окно
- Vulkan validation layers включены в debug-билде, выключены в release
- Шейдеры лежат в `assets/shaders/`, загружаются через `brave_assets`

### Источники света

Четыре типа:

```rust
// Направленный свет (солнце)
DirectionalLight {
    color: Color,
    intensity: f32,
    shadows: bool,       // shadow maps
}

// Точечный свет (лампочка)
PointLight {
    color: Color,
    intensity: f32,
    range: f32,
}

// Прожектор
SpotLight {
    color: Color,
    intensity: f32,
    range: f32,
    angle: f32,          // угол конуса
}

// Фоновый свет
AmbientLight {
    color: Color,
    intensity: f32,
}
```

### Shadow maps

Тени только от `DirectionalLight` с флагом `shadows: true`. Реализация через depth-only render pass в отдельный framebuffer.

### Skybox

Устанавливается через рендерер:

```rust
let skybox_tex = game.assets.load_asset("sky", AssetFormat::Texture);
game.render.set_skybox(skybox_tex);
```

### Camera

```rust
Camera {
    fov: f32,       // угол обзора в градусах
    near: f32,      // ближняя плоскость
    far: f32,       // дальняя плоскость
}
```

Камера — компонент на сущности. Рендерер находит сущность с `Camera` и `Transform` и использует для построения матриц.

### ImGui Vulkan рендерер

Свой рендерер для imgui-rs draw data. ImGui отдаёт вертексные/индексные буферы и команды отрисовки — мы рисуем их поверх основной сцены в отдельном render pass.

---

## Модуль brave_assets

### Общее

Одна функция загрузки, без кеша:

```rust
game.assets.load_asset("player", AssetFormat::Model)    // .glb/.gltf
game.assets.load_asset("albedo", AssetFormat::Texture)   // .png/.jpg/.hdr
game.assets.load_asset("mesh", AssetFormat::Shader)      // .glsl → SPIR-V
```

`AssetFormat` — enum:

```rust
pub enum AssetFormat {
    Model,    // загружает из models.ast (release) или assets/models/ (dev)
    Texture,  // загружает из textures.ast (release) или assets/textures/ (dev)
    Shader,   // загружает из shaders.ast (release) или assets/shaders/ (dev)
}
```

### Формат .ast

Бинарный формат для релизных билдов. Упаковка пачками по типу:

- `models.ast` — все модели
- `textures.ast` — все текстуры
- `shaders.ast` — все шейдеры (уже скомпилированные в SPIR-V)

Структура файла:

```
[4 байта]  Магическое число: "BRAV"
[4 байта]  Версия формата
[4 байта]  Количество записей (N)
[N записей] Таблица:
    [64 байта]  Имя ассета (UTF-8, null-terminated)
    [4 байта]   Тип ассета
    [8 байт]    Смещение данных от начала файла
    [8 байт]    Размер данных
[данные]   Сырые данные подряд
```

Данные в .ast уже в финальном виде:
- Текстуры — raw RGBA пиксели
- Шейдеры — SPIR-V байткод
- Модели — vertex/index буферы в нужном layout

### Конвертация

Конвертация из исходников в .ast происходит при сборке через отдельную утилиту или `build.rs`. В dev-режиме (cargo run) движок читает исходники напрямую из `assets/`.

### Лоадеры

Каждый формат имеет свой лоадер:

- `GltfLoader` — парсит .glb/.gltf, извлекает меши, материалы, текстуры. Крейт: `gltf`
- `ImageLoader` — загружает .png/.jpg/.hdr. Крейт: `image`
- `ShaderLoader` — компилирует .glsl в SPIR-V. Крейт: `shaderc` или `naga`

---

## Модуль brave_scene

### Transform

Основной компонент позиционирования:

```rust
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,    // или Vec3 для euler angles
    pub scale: Vec3,
}

Transform::new(x, y, z)   // position, rotation=identity, scale=1
```

### Hierarchy (parent-child)

Поддержка иерархии трансформов:

```rust
game.world.spawn("arm")
    .with(Transform::new(1.0, 0.0, 0.0))
    .child_of("player");    // наследует трансформ от parent
```

### Сцена

Сцена описывается полностью в Rust-коде. Нет файлов сцен. Все объекты создаются через `game.world.spawn()` в startup-системе.

---

## Модуль brave_input

Polling API, только клавиатура и мышь:

```rust
// Клавиатура
game.input.is_key_pressed(Key::Space)   // нажата в этом кадре
game.input.is_key_held(Key::W)          // зажата
game.input.is_key_released(Key::Escape) // отпущена в этом кадре

// Мышь
game.input.mouse_position()             // (x, y)
game.input.mouse_delta()                // (dx, dy) движение за кадр
game.input.is_mouse_pressed(MouseButton::Left)
game.input.is_mouse_held(MouseButton::Right)
```

Интеграция с winit — события приходят из event loop, Input обновляется каждый кадр перед системами.

---

## Модуль brave_window

Обёртка над `winit`. Одно окно.

```rust
WindowPlugin {
    title: &str,
    width: u32,
    height: u32,
}

game.window.width()                     // текущая ширина
game.window.height()                    // текущая высота
game.window.quit()                      // закрыть окно и завершить
game.window.set_title("New Title")
game.window.set_fullscreen(bool)
game.window.set_cursor_visible(bool)
game.window.set_cursor_grabbed(bool)    // захват курсора (для FPS)
```

---

## Модуль brave_math

Обёртка-реэкспорт над `glam`:

```rust
pub use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
```

Плюс утилиты:

```rust
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

Color::WHITE
Color::BLACK
Color::RED
Color::new(r, g, b, a)
```

---

## HUD система (game.ui)

### Общее

Используется `imgui-rs` со своим Vulkan рендерером. HUD рисуется поверх 3D сцены.

### Трейт HudElement

```rust
pub trait HudElement: 'static {
    fn render(&mut self, ui: &imgui::Ui, game: &Engine);
    fn update(&mut self, game: &Engine) {}   // опционально, каждый кадр
    fn visible(&self) -> bool { true }
}
```

Каждый HUD элемент — структура, реализующая `HudElement`. Внутри `render()` пишется imgui-код напрямую:

```rust
struct FpsCounter {
    fps: u32,
    frame_count: u32,
    last_time: f64,
}

impl HudElement for FpsCounter {
    fn render(&mut self, ui: &imgui::Ui, game: &Engine) {
        ui.window("FPS")
            .no_title_bar(true)
            .always_auto_resize(true)
            .position(
                [game.window.width() as f32 - 80.0, 10.0],
                imgui::Condition::Always,
            )
            .build(|| {
                ui.text(format!("FPS: {}", self.fps));
            });
    }

    fn update(&mut self, game: &Engine) {
        self.frame_count += 1;
        let now = game.time.elapsed();
        if now - self.last_time >= 0.5 {
            self.fps = (self.frame_count as f64 / (now - self.last_time)) as u32;
            self.frame_count = 0;
            self.last_time = now;
        }
    }
}
```

### API game.ui

```rust
game.ui.add_element(impl HudElement)     // добавить элемент
game.ui.remove_element::<FpsCounter>()   // убрать по типу
game.ui.get_element::<FpsCounter>()      // получить ссылку
game.ui.toggle::<FpsCounter>()           // переключить visible
game.ui.show::<FpsCounter>()             // показать
game.ui.hide::<FpsCounter>()             // скрыть
game.ui.is_hovered()                     // курсор над UI элементом
```

---

## Коллизии

Базовая система на старте:

```rust
// AABB (axis-aligned bounding box)
game.physics.aabb_intersects(entity_a, entity_b) -> bool

// Raycast
game.physics.raycast(origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayHit>

pub struct RayHit {
    pub entity: &Entity,
    pub point: Vec3,       // точка попадания
    pub distance: f32,     // расстояние
    pub normal: Vec3,      // нормаль поверхности
}
```

Полноценная физика (rapier3d) — позже.

---

## Логирование

`env_logger` — логи в консоль:

```rust
// в движке
log::info!("Engine started");
log::warn!("Shader compilation warning: ...");
log::error!("Failed to load asset: ...");

// уровни: BRAVE_LOG=debug cargo run
```

---

## Vulkan отладка

В debug-билде автоматически включаются Vulkan validation layers (`VK_LAYER_KHRONOS_validation`). Ошибки Vulkan API выводятся в консоль через debug callback. В release-билде validation layers отключены.

---

## Внешние зависимости (крейты)

| Крейт | Назначение |
|---|---|
| `ash` | Vulkan API биндинги |
| `winit` | Управление окном |
| `glam` | Математика (Vec3, Mat4, Quat) |
| `gltf` | Загрузка .glb/.gltf моделей |
| `image` | Загрузка текстур (png, jpg, hdr) |
| `shaderc` или `naga` | Компиляция GLSL → SPIR-V |
| `imgui-rs` | Dear ImGui биндинги |
| `gpu-allocator` | Выделение GPU памяти |
| `env_logger` | Логирование |
| `log` | Фасад логирования |

---

## Пример игры (3 файла)

### src/main.rs

```rust
use brave_core::prelude::*;

mod game;
mod hud;

fn main() {
    let mut game = Engine::new();

    game.add_plugin(WindowPlugin {
        title: "My Game",
        width: 1280,
        height: 720,
    });
    game.add_plugin(RenderPlugin);
    game.add_plugin(InputPlugin);
    game.add_plugin(AssetPlugin::new("assets/"));

    game.add_startup_system(game::setup);
    game.add_system(game::global_controls);

    game.run();
}
```

### src/game.rs

```rust
use brave_core::prelude::*;
use crate::hud::FpsCounter;

pub fn setup(game: &mut Engine) {
    // загрузка ассетов
    let player_mesh = game.assets.load_asset("player", AssetFormat::Model);
    let ground_mesh = game.assets.load_asset("ground", AssetFormat::Model);
    let skybox_tex = game.assets.load_asset("sky", AssetFormat::Texture);

    // игрок с тик-скриптом
    game.world.spawn("player")
        .with(Transform::new(0.0, 1.0, 0.0))
        .with(MeshRenderer::new(player_mesh))
        .with(Script::new(player_update));

    // земля без скрипта
    game.world.spawn("ground")
        .with(Transform::new(0.0, 0.0, 0.0))
        .with(MeshRenderer::new(ground_mesh));

    // камера
    game.world.spawn("camera")
        .with(Transform::new(0.0, 5.0, -10.0))
        .with(Camera {
            fov: 60.0,
            near: 0.1,
            far: 1000.0,
        });

    // свет
    game.world.spawn("sun")
        .with(Transform::new(0.0, 10.0, 0.0))
        .with(DirectionalLight {
            color: Color::WHITE,
            intensity: 1.0,
            shadows: true,
        });

    // skybox
    game.render.set_skybox(skybox_tex);

    // HUD
    game.ui.add_element(FpsCounter::new());
}

// скрипт игрока — вызывается каждый fixed tick (60fps)
fn player_update(entity: &mut Entity, game: &Engine) {
    let transform = entity.get_mut::<Transform>();
    let speed = 5.0 * game.time.fixed_delta();

    if game.input.is_key_held(Key::W) { transform.position.z += speed; }
    if game.input.is_key_held(Key::S) { transform.position.z -= speed; }
    if game.input.is_key_held(Key::A) { transform.position.x -= speed; }
    if game.input.is_key_held(Key::D) { transform.position.x += speed; }

    if game.input.is_key_pressed(Key::Space) {
        transform.position.y += 2.0;
    }
}

// глобальная система — вызывается каждый кадр (variable dt)
pub fn global_controls(game: &mut Engine) {
    if game.input.is_key_pressed(Key::Escape) {
        game.window.quit();
    }

    if game.input.is_key_pressed(Key::B) {
        game.world.despawn("ground");
    }
}
```

### src/hud.rs

```rust
use brave_core::prelude::*;

pub struct FpsCounter {
    fps: u32,
    frame_count: u32,
    last_time: f64,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            fps: 0,
            frame_count: 0,
            last_time: 0.0,
        }
    }
}

impl HudElement for FpsCounter {
    fn render(&mut self, ui: &imgui::Ui, game: &Engine) {
        ui.window("FPS")
            .no_title_bar(true)
            .always_auto_resize(true)
            .position(
                [game.window.width() as f32 - 80.0, 10.0],
                imgui::Condition::Always,
            )
            .build(|| {
                ui.text(format!("FPS: {}", self.fps));
            });
    }

    fn update(&mut self, game: &Engine) {
        self.frame_count += 1;
        let now = game.time.elapsed();
        if now - self.last_time >= 0.5 {
            self.fps = (self.frame_count as f64 / (now - self.last_time)) as u32;
            self.frame_count = 0;
            self.last_time = now;
        }
    }
}
```

---

## Порядок реализации (рекомендуемый)

1. `brave_math` — реэкспорт glam, Color
2. `brave_window` — winit окно, event loop
3. `brave_input` — polling ввода из winit events
4. `brave_core` — Engine struct, Plugin trait, game loop, Time
5. `brave_ecs` — World, Entity, Component, spawn/despawn, Script
6. `brave_render` — Vulkan init, swapchain, простой pipeline, меш рендеринг
7. `brave_assets` — load_asset, лоадеры (gltf, image, shader)
8. `brave_scene` — Transform, hierarchy
9. Освещение — 4 типа источников + shadow maps
10. Skybox
11. HUD — imgui-rs интеграция, свой Vulkan рендерер, HudElement трейт
12. Коллизии — AABB, raycast
13. Формат .ast — конвертация, упаковка
