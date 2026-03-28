# Инфраструктура

## Логирование

Используется `env_logger` + `log` — стандартная связка в Rust.

```rust
// В коде движка
log::info!("Engine started");
log::warn!("Shader compilation warning: {}", msg);
log::error!("Failed to load asset: {}", path);
log::debug!("Frame time: {:.2}ms", dt * 1000.0);
```

Уровень логирования задаётся через переменную окружения:

```bash
BRAVE_LOG=debug cargo run     # все сообщения включая debug
BRAVE_LOG=info cargo run      # info, warn, error
BRAVE_LOG=warn cargo run      # только предупреждения и ошибки
BRAVE_LOG=error cargo run     # только ошибки
```

По умолчанию (без переменной) — `info`.

---

## Vulkan отладка

В **debug-билде** автоматически включаются Vulkan validation layers (`VK_LAYER_KHRONOS_validation`). Они ловят:

- Неправильные параметры Vulkan API
- Утечки GPU-памяти
- Некорректные состояния pipeline
- Ошибки синхронизации

Ошибки выводятся в консоль через Vulkan debug callback, интегрированный с `log` крейтом — ошибки Vulkan появляются в том же логе что и сообщения движка.

В **release-билде** validation layers полностью отключены — ноль overhead.

---

## Внешние зависимости (крейты)

| Крейт | Назначение | Используется в |
|---|---|---|
| `ash` | Vulkan API биндинги | brv_render |
| `winit` | Управление окном, event loop | brv_window, brv_input |
| `glam` | Математика (Vec3, Mat4, Quat, SIMD) | brv_math |
| `gltf` | Загрузка .glb/.gltf моделей | brv_assets |
| `image` | Декодирование текстур (PNG, JPG, HDR) | brv_assets |
| `shaderc` или `naga` | Компиляция GLSL → SPIR-V | brv_assets |
| `gpu-allocator` | Выделение GPU памяти (Vulkan Memory Allocator) | brv_render |
| `toml` | Парсинг astdb.lock | brv_assets |
| `env_logger` | Логирование в консоль | brv_core |
| `log` | Фасад логирования (макросы info!/warn!/error!) | все крейты |