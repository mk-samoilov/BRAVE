# Модуль brv_render

## Общее

- Vulkan через крейт `ash`
- Forward rendering с PBR (metallic/roughness)
- Одно окно
- Vulkan validation layers автоматически включены в debug-билде, выключены в release

## Референс

Система рендеринга и освещения строится на основе [Ash-Renderer](https://github.com/saptak7777/Ash-Renderer) — Vulkan рендерер на Rust/ash с PBR, Cascaded Shadow Maps (CSM), Screen-Space Global Illumination (SSGI), GPU frustum culling, bindless текстурами, bloom и tonemapping.

Берём технику освещения и теней из этого проекта, но **полностью переписываем** под архитектуру BRAVE: интеграция с нашим ECS (Object API), plugin system, asset pipeline. Не копируем код — используем как референс подходов и алгоритмов.

---

## Camera

```rust
Camera {
fov: f32,       // угол обзора в градусах
near: f32,      // ближняя плоскость отсечения
far: f32,       // дальняя плоскость отсечения
}
```

Camera — Option-поле на объекте. Рендерер каждый кадр ищет объект с `camera != None` и берёт из него `transform` (позиция) + `rotate` (направление взгляда) + `camera` (fov/near/far) для построения матриц view и projection.

> **Одна активная камера**: если на сцене несколько объектов с `camera != None` одновременно — runtime паника с сообщением `"Multiple cameras found: only one camera allowed"`. Для переключения камер нужно сначала убрать компонент у текущей: `old_cam.camera = None;`, затем установить новой.

```rust
let cam = game.world.spawn("camera");
cam.transform.set(0.0, 5.0, -10.0);
cam.rotate.look_at_vec(Vec3::ZERO);
cam.camera.set(Camera { fov: 60.0, near: 0.1, far: 1000.0 });
```

---

## PBR Material

Модель освещения — PBR (Physically Based Rendering) с metallic/roughness workflow. Это индустриальный стандарт, который напрямую совместим с glTF форматом — Blender экспортирует модели именно с PBR-материалами.

Параметры материала:
- **albedo** — базовый цвет (текстура или solid color)
- **metallic** — металличность (0.0 = диэлектрик, 1.0 = металл)
- **roughness** — шероховатость (0.0 = зеркало, 1.0 = матовый)
- **normal map** — карта нормалей (опционально)
- **emissive** — самосвечение (опционально)

Материалы извлекаются автоматически при загрузке glTF/GLB моделей через brv_assets.

---

## Освещение

### Четыре типа источников света

Каждый источник — один на объект. Количество источников на сцену не ограничено. Тени всегда включены для всех типов (кроме Ambient).

#### DirectionalLight — направленный свет (солнце)

Свет из бесконечности в одном направлении. Направление берётся из `rotate` объекта, к которому привязан свет. Позиция не важна.

```rust
DirectionalLight {
color: Color,        // цвет света
intensity: f32,      // интенсивность
}
```

Тени: **Cascaded Shadow Maps (CSM)** — shadow map разбивается на несколько каскадов по расстоянию от камеры. Ближние объекты получают высокое разрешение теней, дальние — низкое. Это стандартная техника для солнечного света в open-world играх.

```rust
let sun = game.world.spawn("sun");
sun.transform.set(0.0, 100.0, 0.0);
sun.rotate.set(-0.5, 0.0, 0.0);  // направление вниз-вперёд
sun.light.set(DirectionalLight { color: Color::DAYLIGHT, intensity: 1.0 });
```

#### PointLight — точечный свет (лампочка)

Свет из точки во все стороны. Позиция берётся из `transform` объекта. Затухает с расстоянием.

```rust
PointLight {
color: Color,        // цвет света
intensity: f32,      // интенсивность
range: f32,          // радиус действия (за пределами — нет освещения)
}
```

Тени: **кубические shadow maps** — рендерим сцену 6 раз (по одному на каждую грань куба) из позиции источника. Это позволяет отбрасывать тени во все стороны.

```rust
let lamp = game.world.spawn("lamp");
lamp.transform.set(3.0, 2.5, 0.0);
lamp.light.set(PointLight { color: Color::WARM, intensity: 5.0, range: 10.0 });
```

#### SpotLight — прожектор

Свет конусом из точки в направлении. Позиция из `transform`, направление из `rotate`.

```rust
SpotLight {
color: Color,        // цвет света
intensity: f32,      // интенсивность
range: f32,          // дальность
angle: f32,          // **half-angle** конуса в градусах: angle=30 → конус 60° итого (как в Blender/Unity)
}
```

Тени: **перспективные shadow maps** — один render pass с перспективной проекцией, совпадающей с конусом прожектора.

```rust
let flash = game.world.spawn("flashlight");
flash.transform.set(0.0, 1.5, 0.0);
flash.rotate.set(0.0, 0.0, 0.0);  // направление вперёд
flash.light.set(SpotLight { color: Color::WHITE, intensity: 8.0, range: 20.0, angle: 30.0 });
```

#### AmbientLight — фоновый свет

Равномерный свет со всех сторон. Не имеет позиции и направления. Без теней.

```rust
AmbientLight {
color: Color,        // цвет
intensity: f32,      // интенсивность
}
```

```rust
let ambient = game.world.spawn("ambient");
ambient.light.set(AmbientLight { color: Color::COOL, intensity: 0.3 });
```

---

## Shadow Maps — детали реализации

### Cascaded Shadow Maps (CSM) для DirectionalLight

1. Пространство перед камерой разбивается на N каскадов (обычно 3-4) по логарифмической шкале глубины
2. Для каждого каскада строится ортографическая проекция из направления света
3. Сцена рендерится в depth-only pass для каждого каскада
4. В основном шейдере выбирается подходящий каскад по глубине фрагмента
5. Ближние каскады — высокое разрешение, дальние — низкое

### Кубические shadow maps для PointLight

1. Из позиции источника рендерим сцену 6 раз (±X, ±Y, ±Z) в faces cubemap
2. В основном шейдере сэмплируем cubemap по направлению от источника к фрагменту
3. Оптимизация: рендерим тени только для источников в пределах frustum камеры

### Перспективные shadow maps для SpotLight

1. Строим перспективную проекцию с углом = angle прожектора
2. Один depth-only render pass из позиции источника в направлении конуса
3. В основном шейдере проецируем фрагмент в пространство shadow map

---

## Оптимизация рендера

- **Frustum culling** — объекты вне видимости камеры не отправляются на GPU. Проверка AABB объекта против frustum камеры на CPU.
- **Batching** — группировка draw calls по материалу и мешу. Объекты с одинаковым материалом и мешом рисуются одним instanced draw call.
- **Переиспользование command buffers** — command buffers записываются и переиспользуются между кадрами, перезаписываются только при изменении сцены.
- **Shadow map culling** — тени рендерятся только для источников света, попадающих в frustum камеры.

---

## Vulkan отладка

В debug-билде автоматически включаются Vulkan validation layers (`VK_LAYER_KHRONOS_validation`). Ошибки Vulkan API (неправильные параметры, утечки памяти, некорректные состояния) выводятся в консоль через debug callback. В release-билде validation layers полностью отключены — ноль overhead.

---

## Ray Tracing (Path Tracing)

### Общее

Движок поддерживает два режима рендеринга:

- **Rasterization** — forward PBR с shadow maps (описан выше). Работает на любой Vulkan GPU.
- **Path Tracing** — полный ray tracing через `VK_KHR_ray_tracing_pipeline`. Требует GPU с аппаратной поддержкой RT (NVIDIA RTX 20xx+, AMD RDNA2+).

Режим определяется автоматически при старте: движок проверяет поддержку расширений `VK_KHR_ray_tracing_pipeline`, `VK_KHR_acceleration_structure`, `VK_KHR_ray_query`. Если все есть — включается path tracing. Если нет — fallback на rasterization.

### Референс

Реализация строится на основе [ash-raytracing-example](https://github.com/hatoo/ash-raytracing-example) — минимальный пример KHR ray tracing на ash. Берём как стартовую точку для pipeline setup, shader binding table, acceleration structures. Переписываем под архитектуру BRAVE.

### Переключение в рантайме

```rust
// Переключение режима рендеринга
game.render.set_mode(RenderMode::PathTracing);      // включить RT
game.render.set_mode(RenderMode::Rasterization);    // включить rasterization
game.render.get_mode() -> RenderMode;               // текущий режим

// Проверка поддержки
game.render.supports_ray_tracing() -> bool;          // поддерживает ли GPU
```

Если вызвать `set_mode(PathTracing)` на GPU без поддержки RT — runtime ошибка с понятным сообщением. Рекомендуется проверять `supports_ray_tracing()` перед переключением.

### RenderMode enum

```rust
pub enum RenderMode {
    Rasterization,   // forward PBR + shadow maps
    PathTracing,     // полный ray tracing
}
```

### Архитектура RT pipeline

#### Acceleration Structures (AS)

Двухуровневая иерархия:

- **BLAS (Bottom-Level AS)** — одна на каждый уникальный меш. Содержит геометрию (vertex/index buffers). Создаётся при загрузке модели и переиспользуется между объектами с одинаковым мешом.
- **TLAS (Top-Level AS)** — одна на всю сцену. Содержит ссылки на BLAS с трансформами каждого объекта. Пересобирается каждый кадр (или при изменении сцены).

#### Shader Binding Table (SBT)

Таблица привязки шейдеров, определяющая какой шейдер вызывается на каком этапе:

| Тип шейдера | Назначение |
|---|---|
| Ray Generation | Запускает лучи из камеры. Один на пиксель. |
| Miss | Вызывается когда луч ничего не задел (sky/environment). |
| Closest Hit | Вызывается при попадании луча в ближайшую геометрию. Считает PBR освещение, запускает вторичные лучи (отражения, тени, GI). |
| Any Hit | Опционально. Для прозрачных объектов, alpha test. |

#### Path Tracing алгоритм

1. **Ray Generation** — для каждого пикселя генерируется первичный луч из камеры
2. **Trace** — луч проходит через TLAS → BLAS, находит ближайшее пересечение
3. **Closest Hit** — в точке пересечения:
    - Вычисляется PBR shading (albedo, metallic, roughness, normal)
    - Запускаются shadow rays к каждому источнику света
    - Запускаются лучи отражения/преломления (по материалу)
    - Запускаются лучи для Global Illumination (случайные направления)
4. **Miss** — луч ушёл в небо → возвращает цвет skybox/environment
5. **Accumulation** — результаты нескольких кадров накапливаются для шумоподавления. При движении камеры — сброс аккумуляции.

#### Количество bounces

По умолчанию:
- Shadow rays: 1 bounce (прямая видимость до источника)
- Reflection: до 4 bounces
- GI: 2 bounces

Настраивается:

```rust
game.render.set_max_bounces(8);          // максимум переотражений
game.render.set_samples_per_pixel(4);    // количество лучей на пиксель за кадр
```

### Интеграция с существующей системой освещения

В режиме Path Tracing **те же самые** 4 типа источников света (DirectionalLight, PointLight, SpotLight, AmbientLight) работают автоматически. Разница только в реализации:

| | Rasterization | Path Tracing |
|---|---|---|
| Тени | Shadow maps (CSM, cube, perspective) | Shadow rays (точные, без артефактов) |
| Отражения | Нет (или SSR позже) | Настоящие ray-traced отражения |
| GI | Нет (только AmbientLight) | Полное глобальное освещение |
| Мягкие тени | Нет (резкие края) | Естественные мягкие тени |
| Каустики | Нет | Возможны через path tracing |

Пользовательский код не меняется — одна и та же сцена рендерится обоими способами. Переключение прозрачное.

### Оптимизация RT

- **BLAS переиспользование** — объекты с одинаковым мешом разделяют BLAS
- **TLAS incremental update** — при движении одного объекта перестраивается только его instance в TLAS, а не вся структура
- **Sample accumulation** — при статичной камере кадры накапливаются, качество растёт со временем
- **Denoising** — в будущем: шумоподавление для уменьшения количества лучей на пиксель при сохранении качества

### Vulkan расширения для RT

```
VK_KHR_ray_tracing_pipeline        // ray tracing pipeline
VK_KHR_acceleration_structure       // BLAS/TLAS
VK_KHR_ray_query                    // inline ray tracing в обычных шейдерах
VK_KHR_deferred_host_operations     // async AS build
VK_KHR_buffer_device_address        // device pointers для SBT
```

Все проверяются при старте. Если хотя бы одного нет — fallback на rasterization.
