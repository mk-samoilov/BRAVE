# Модуль brv_core

## Engine

`Engine` — центральный объект движка. Один и тот же тип используется везде: при создании в `main()` и в системах как параметр `game`.

Поля являются `Option<T>` — до вызова соответствующего плагина поле = `None`. При обращении к незаполненному полю — паника с понятным сообщением (например `"WindowPlugin not loaded"`). Это гарантирует что забытый плагин обнаруживается сразу при первом обращении, а не молча падает.

```rust
pub struct Engine {
    pub world: World,                    // ECS (всегда доступен, создаётся в Engine::new())
    pub assets: Option<AssetManager>,    // загрузка ассетов — заполняется AssetPlugin
    pub input: Option<Input>,            // клавиатура, мышь — заполняется InputPlugin
    pub render: Option<Renderer>,        // Vulkan рендерер — заполняется RenderPlugin
    pub time: Time,                      // delta time (всегда доступен, создаётся в Engine::new())
    pub window: Option<Window>,          // окно — заполняется WindowPlugin
}
```

### Методы верхнего уровня

```rust
Engine::new()                          // создать движок (world и time создаются сразу)
game.add_plugin(plugin)                // подключить плагин (вызывает plugin.build(&mut self))
game.add_startup_system(fn)            // система один раз при старте, до основного цикла
game.add_system(fn)                    // система каждый кадр в variable update
game.remove_system(fn)                 // убрать систему по указателю на функцию
game.run()                             // запустить game loop (забирает управление, не возвращается)
```

### Пример main.rs

```rust
use brv_core::prelude::*;

fn main() {
    let mut game = Engine::new();

    game.add_plugin(WindowPlugin {
        title: "My Game",
        width: 1280,
        height: 720,
    });
    game.add_plugin(RenderPlugin);
    game.add_plugin(InputPlugin);
    game.add_plugin(AssetPlugin::new("assets/", "my_xor_key"));

    game.add_startup_system(setup);
    game.add_system(global_controls);

    game.run();
}
```

---

## Plugin system

Каждый крейт движка экспортирует свой плагин — структуру, реализующую трейт `Plugin`:

```rust
pub trait Plugin: 'static {
    fn build(&self, game: &mut Engine);
}
```

Внутри `build` плагин заполняет соответствующее поле Engine, регистрирует свои внутренние системы, выделяет ресурсы.

### Пример плагина (brv_render)

```rust
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, game: &mut Engine) {
        let window = game.window.as_ref().expect("WindowPlugin not loaded");
        let context = VulkanContext::new(window);
        game.render = Some(Renderer::new(context));
    }
}
```

### Защита от дублей

При `add_plugin` движок проверяет `TypeId` плагина. Если плагин с таким типом уже зарегистрирован — игнорируется. Это защищает от случайной двойной инициализации.

### Порядок важен

Порядок вызова `add_plugin` определяет порядок инициализации. Например, `RenderPlugin` обращается к `game.window` — значит `WindowPlugin` должен быть добавлен раньше. Нарушение порядка → паника с понятным сообщением.

---

## Prelude

`brv_core::prelude` реэкспортирует всё из всех крейтов движка. Один импорт в начале файла — и все типы доступны:

```rust
// brv_core/src/prelude.rs
pub use crate::{Engine, Plugin, Time};
pub use brv_ecs::*;
pub use brv_render::{RenderPlugin, Renderer, MeshRenderer, Camera};
pub use brv_render::{DirectionalLight, PointLight, SpotLight, AmbientLight};
pub use brv_assets::{AssetPlugin, AssetManager, AssetType};
pub use brv_scene::{Transform};
pub use brv_input::{InputPlugin, Input, Key, MouseButton};
pub use brv_window::{WindowPlugin, Window};
pub use brv_math::*;
pub use brv_colors::*;
```

---

## Game loop

Двойной цикл реализуется внутри winit event loop через `AboutToWait` callback. winit захватывает управление через `event_loop.run(...)` — это callback-based. Наш game loop живёт внутри этого callback.

### Два таймера

- **Render rate** (по умолчанию 144 FPS) — рендер, обычные системы (зарегистрированные через `add_system`)
- **Physics rate** (по умолчанию 60 тиков/сек) — Script-функции на объектах. Физика пока не используется, но таймер уже есть — в будущем к нему подключится физический движок.

### Аккумуляторный паттерн

```
// внутри winit AboutToWait callback:

let dt = вычислить_delta_time();
accumulator += dt;

// Fixed update (physics rate, по умолчанию 60fps)
// Скрипты объектов выполняются с фиксированным шагом
while accumulator >= physics_dt {
    for каждый_объект_со_скриптом {
        script_fn(object, engine);
    }
    accumulator -= physics_dt;
}

// Variable update (каждый кадр, до render rate)
for система in системы {
    система(engine);
}

// Рендер
renderer.draw_frame();
```

Фиксированный шаг гарантирует что логика в скриптах работает одинаково независимо от FPS. Переменный шаг для систем позволяет реагировать на события каждый кадр.

---

## Системы

### Два типа регистрации

- `add_startup_system(fn)` — вызывается один раз при старте, до входа в основной цикл. Используется для инициализации сцены: спавн объектов, загрузка ассетов, настройка камеры.
- `add_system(fn)` — вызывается каждый кадр в variable update. Используется для глобальной логики: обработка ввода, переключение состояний, выход из игры.

### Сигнатура

```rust
fn my_system(game: &mut Engine) { ... }
```

Система получает мутабельную ссылку на весь движок — может делать всё: спавнить объекты, загружать ассеты, менять настройки окна.

### Динамическое добавление/удаление

Системы можно добавлять и удалять в рантайме из других систем:

```rust
fn some_system(game: &mut Engine) {
    if game.input.as_ref().unwrap().is_key_pressed(Key::Tab) {
        game.add_system(debug_overlay);  // добавить новую систему
    }
    if game.input.as_ref().unwrap().is_key_pressed(Key::F1) {
        game.remove_system(debug_overlay);  // убрать систему
    }
}
```

Движок хранит системы в `Vec`. `add_system` добавляет в конец, `remove_system` убирает по указателю на функцию. Порядок вызова = порядок добавления.

---

## Time

```rust
pub struct Time { ... }

// Чтение
game.time.delta() -> f32             // переменный dt текущего кадра (секунды)
game.time.fixed_delta() -> f32       // фиксированный dt = 1.0 / physics_rate
game.time.elapsed() -> f64           // секунды с момента запуска движка
game.time.fps() -> f32               // текущий реальный FPS

// Настройка
game.time.set_fps(144.0)             // целевой render rate (по умолчанию 144.0)
game.time.set_physics_rate(60.0)     // целевой physics rate (по умолчанию 60.0)
```

`set_fps` ограничивает частоту кадров сверху (через sleep или vsync). `set_physics_rate` определяет частоту вызова Script-функций и будущего физического движка. Оба значения можно менять в рантайме.
