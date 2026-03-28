# Порядок реализации (рекомендуемый)

Модули выстроены от фундамента к надстройкам. Каждый следующий шаг зависит от предыдущих.

---

## Этап 1 — Фундамент

### 1. `brv_math`
Реэкспорт glam типов (Vec2, Vec3, Vec4, Mat4, Quat). Просто `pub use`. Занимает 5 минут, но нужен всем остальным крейтам.

### 2. `brv_colors`
Структура Color, конструктор `new()`, базовые пресеты (WHITE, BLACK, RED, GREEN, BLUE) и световые пресеты (WARM, COOL, DAYLIGHT, SUNSET).

### 3. `brv_window`
Winit окно + event loop. WindowPlugin создаёт окно с заданным title/width/height. Обработка Resized, CloseRequested. На этом этапе должно открываться пустое окно.

### 4. `brv_input`
Polling ввода из winit events. Два массива состояний (current/previous) для pressed/held/released. mouse_position, mouse_delta, mouse_scroll.

---

## Этап 2 — Ядро

### 5. `brv_core`
Engine struct с Option-полями. Plugin trait с TypeId защитой. Game loop внутри winit AboutToWait с двумя таймерами (render rate 144fps, physics rate 60fps). Time struct. Регистрация startup/update систем. На этом этапе: окно открывается, системы вызываются, ввод работает.

### 6. `brv_ecs`
World, Object, TransformField, RotateField, VisibleField. spawn/get_obj/remove_obj. Component trait. Script component с вызовом в fixed update. Builder-паттерн (.with()). На этом этапе: можно спавнить объекты с позицией и скриптами, логика работает.

---

## Этап 3 — Рендеринг

### 7. `brv_render` — базовый
Vulkan init (Instance, Device, Queues через ash). Swapchain. Простой graphics pipeline. Vertex/index buffers. Отрисовка первого треугольника. Validation layers в debug. На этом этапе: цветной треугольник на экране.

### 8. `brv_render` — меши и камера
MeshRenderer component. Camera component. View/projection матрицы из Object transform+rotate+camera. Загрузка и отрисовка 3D мешей. Depth buffer.

### 9. `brv_render` — PBR материалы
PBR metallic/roughness шейдеры. Albedo, metallic, roughness, normal map текстуры. Материалы извлекаются из glTF при загрузке.

### 10. `brv_render` — освещение
Четыре типа источников (DirectionalLight, PointLight, SpotLight, AmbientLight). PBR lighting calculations в fragment shader. На основе техники из [Ash-Renderer](https://github.com/saptak7777/Ash-Renderer), переписанной под нашу архитектуру.

### 11. `brv_render` — тени
Shadow maps для всех типов. CSM (каскады) для Directional. Кубические для Point. Перспективные для Spot. Depth-only render passes.

---

## Этап 4 — Сцена и ассеты

### 12. `brv_scene`
Hierarchy (parent-child через child_of). Dirty flags на трансформах. Пересчёт мировых матриц. Интеграция с рендером.

### 13. `brv_assets` — debug режим
AssetManager, AssetPlugin. Загрузка из файлов: GltfLoader, GlbLoader, ImageLoader, ShaderLoader. LRU кеш с лимитом GPU-памяти. На этом этапе: можно загружать модели из Blender и видеть их на экране.

### 14. `brv_assets` — release режим
Packer (build.rs): обход assets/, конвертация, упаковка в .ast файлы по 2.5 ГБ. Генерация astdb.lock в TOML. XOR шифрование. Загрузчик release-режима: расшифровка, seek+read, upload на GPU.

---

## Этап 5 — Ray Tracing

### 15. `brv_render` — RT pipeline
Проверка поддержки расширений. Создание acceleration structures (BLAS для мешей, TLAS для сцены). Shader Binding Table. Ray generation + closest hit + miss шейдеры. На основе [ash-raytracing-example](https://github.com/hatoo/ash-raytracing-example), переписанного под архитектуру BRAVE.

### 16. `brv_render` — RT интеграция
RenderMode enum (PathTracing / Rasterization). game.render.set_mode() / get_mode(). Автодетект при старте. Fallback на rasterization. Shadow rays, reflection rays, GI. Sample accumulation.

### 17. `brv_render` — RT оптимизация
BLAS переиспользование. TLAS incremental update. Настройка bounces и samples per pixel.

---

## Результат

После всех этапов: полноценный мини-движок с Vulkan рендерингом, PBR освещением, тенями, ray tracing с fallback на rasterization, загрузкой моделей из Blender, системой ассетов с упаковкой и шифрованием, ECS с скриптами, и всё это через удобный API `game.*`.
