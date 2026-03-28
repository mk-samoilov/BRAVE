# Модуль brv_ecs

Свой ECS написанный с нуля. Оптимизация: компоненты хранятся в плотных массивах для cache-friendly доступа.

---

## World

`World` — хранилище всех объектов на сцене. Доступен через `game.world`.

```rust
// Создание объекта с уникальным именем
let player = game.world.spawn("player");

// Если имя уже занято — runtime ошибка (паника)
// game.world.spawn("player");  // ПАНИКА: "Object 'player' already exists"

// Поиск объекта по имени
let obj = game.world.get_obj("player");  // -> Option<&Object>

// Удаление объекта по имени
game.world.remove_obj("player");
```

Имена задаются вручную — это строковые идентификаторы типа `"player"`, `"enemy_1"`, `"sun"`. Не UUID, а человекочитаемые имена.

---

## Object

Каждый объект (Object) имеет встроенные поля, которые есть всегда, и Option-поля, которые `None` по умолчанию.

```rust
pub struct Object {
    // === Встроенные (всегда есть у каждого объекта) ===
    pub transform: TransformField,   // позиция в мире, по умолчанию (0, 0, 0)
    pub rotate: RotateField,         // вращение, по умолчанию identity (нет вращения)
    pub visible: VisibleField,       // видимость, по умолчанию true

    // === Option (None по умолчанию, заполняются через .set()) ===
    pub mesh: Option<MeshComponent>,     // 3D модель
    pub camera: Option<Camera>,          // камера
    pub light: Option<Light>,            // источник света (один на объект)
    pub script: Option<Script>,          // тик-функция
}
```

### Встроенные поля

Эти поля существуют у каждого объекта с момента создания. Не нужно их "добавлять" — они просто есть.

#### TransformField — позиция

```rust
player.transform.set(0.0, 1.0, 0.0);    // задать позицию (x, y, z)
player.transform.get() -> Vec3;          // получить текущую позицию
```

#### RotateField — вращение

Внутри хранится как `Quat` (кватернион) — без gimbal lock, корректная интерполяция. API принимает и возвращает euler angles в радианах, порядок XYZ.

```rust
player.rotate.set(0.0, 3.14, 0.0);       // задать вращение (радианы, XYZ)
player.rotate.get() -> Vec3;              // получить euler angles обратно

player.rotate.look_at_obj(&enemy);        // повернуться лицом к другому объекту
player.rotate.look_at_vec(Vec3::ZERO);    // повернуться лицом к точке в пространстве
```

Конвертация euler ↔ Quat автоматическая и прозрачная для пользователя.

#### VisibleField — видимость

```rust
player.visible.set(false);    // скрыть объект (не рендерится, но существует)
player.visible.get() -> bool; // проверить видимость
```

Скрытый объект не отправляется на рендер, но его Script продолжает вызываться, и он участвует в логике.

### Option-поля

Эти поля = `None` по умолчанию. Устанавливаются через `.set()`, снимаются через `.clear()` (для Script).

#### Mesh — 3D модель

```rust
let mesh = game.assets.load("models/chair", AssetType::GLTFModel);
player.mesh.set(mesh);
```

Привязывается через уже загруженный ассет (результат `game.assets.load()`).

#### Camera

```rust
player.camera.set(Camera {
    fov: 60.0,     // угол обзора в градусах
    near: 0.1,     // ближняя плоскость отсечения
    far: 1000.0,   // дальняя плоскость отсечения
});
```

Рендерер ищет объект с `camera != None` и использует его transform + camera данные для построения матриц view/projection.

#### Light — источник света

Один источник на объект. Четыре типа (подробнее в [04_render.md](./04_render.md)):

```rust
sun.light.set(DirectionalLight { color: Color::DAYLIGHT, intensity: 1.0 });
lamp.light.set(PointLight { color: Color::WARM, intensity: 5.0, range: 10.0 });
flash.light.set(SpotLight { color: Color::WHITE, intensity: 8.0, range: 20.0, angle: 30.0 });
ambient.light.set(AmbientLight { color: Color::COOL, intensity: 0.3 });
```

#### Script — тик-функция

Подробнее в разделе Script ниже.

```rust
player.script.set(Script::new(player_update));
player.script.clear();   // убрать скрипт
```

### Удаление объекта

Два способа:

```rust
player.remove();                    // через сам объект
game.world.remove_obj("player");    // через world по имени
```

При удалении: объект убирается из world, все GPU-ресурсы (mesh, текстуры) остаются в LRU кеше ассетов (могут быть использованы другими объектами).

### Альтернативный способ через .with()

Вместо пошаговой настройки через поля можно использовать builder-паттерн:

```rust
let player = game.world.spawn("player")
    .with(Transform::new(0.0, 1.0, 0.0))
    .with(Camera { fov: 60.0, near: 0.1, far: 1000.0 })
    .with(Script::new(player_update));
```

Оба способа (поля и `.with()`) работают и могут комбинироваться. `.with()` удобен для инициализации в startup-системе, прямые поля — для изменений в рантайме.

---

## Script

Script — опциональный компонент на объекте. Это функция, которая автоматически вызывается движком каждый fixed tick (physics rate, по умолчанию 60fps).

### Сигнатура

```rust
fn player_update(obj: &mut Object, game: &mut Engine) {
    // obj — этот конкретный объект
    // game — весь движок (мутабельный — можно спавнить, удалять, менять мир)
}
```

### Пример

```rust
fn player_update(obj: &mut Object, game: &mut Engine) {
    let speed = 5.0 * game.time.fixed_delta();
    let pos = obj.transform.get();

    if game.input.as_ref().unwrap().is_key_held(Key::W) {
        obj.transform.set(pos.x, pos.y, pos.z + speed);
    }
    if game.input.as_ref().unwrap().is_key_held(Key::S) {
        obj.transform.set(pos.x, pos.y, pos.z - speed);
    }
    if game.input.as_ref().unwrap().is_key_held(Key::A) {
        obj.transform.set(pos.x - speed, pos.y, pos.z);
    }
    if game.input.as_ref().unwrap().is_key_held(Key::D) {
        obj.transform.set(pos.x + speed, pos.y, pos.z);
    }

    // Можно спавнить новые объекты из скрипта
    if game.input.as_ref().unwrap().is_key_pressed(Key::E) {
        let bullet = game.world.spawn("bullet_123");
        bullet.transform.set(pos.x, pos.y, pos.z);
    }
}
```

### Привязка

```rust
player.script.set(Script::new(player_update));  // привязать
player.script.clear();                           // убрать
```

### Важно

- Script **необязательный**. Объект работает и без него.
- Script вызывается в **fixed update** (physics rate), не в variable update.
- Script получает **`&mut Engine`** — может делать всё что угодно с миром.
- Используй `game.time.fixed_delta()` внутри скриптов для корректного движения.
