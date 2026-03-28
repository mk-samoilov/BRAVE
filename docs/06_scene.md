# Модуль brv_scene

## Transform

Реализован как встроенное поле `TransformField` на каждом Object. Хранит позицию объекта в мировом пространстве.

```rust
obj.transform.set(x, y, z);     // задать позицию
obj.transform.get() -> Vec3;    // получить позицию
```

Transform создаётся автоматически при `spawn()` со значением `(0.0, 0.0, 0.0)`.

---

## Rotation

Реализован как встроенное поле `RotateField` на каждом Object.

### Внутреннее представление

Внутри хранится `Quat` (кватернион). Кватернионы не подвержены gimbal lock и корректно интерполируются (slerp). Пользователь работает с euler angles — конвертация автоматическая.

### API

```rust
obj.rotate.set(x, y, z);                 // задать вращение (радианы, порядок XYZ)
obj.rotate.get() -> Vec3;                // получить euler angles (радианы, XYZ)

obj.rotate.look_at_obj(&other_object);   // повернуться лицом к другому объекту
                                          // берёт позицию другого объекта и вычисляет
                                          // кватернион направления от текущего к цели

obj.rotate.look_at_vec(target: Vec3);    // повернуться лицом к точке в пространстве
```

### Детали

- Углы в **радианах** (не градусах). `std::f32::consts::PI` = 180°
- Порядок осей: **XYZ** (сначала вращение вокруг X, потом Y, потом Z)
- `look_at_obj` берёт `transform.get()` целевого объекта и вызывает внутренний `look_at`
- `look_at_vec` принимает произвольную точку в мировом пространстве
- Rotation по умолчанию = identity (нет вращения)

---

## Hierarchy (parent-child)

Объекты могут быть связаны в иерархию. Дочерний объект наследует трансформ родителя — его position и rotation вычисляются относительно родителя.

```rust
let body = game.world.spawn("body");
body.transform.set(0.0, 1.0, 0.0);

let arm = game.world.spawn("arm");
arm.transform.set(1.0, 0.0, 0.0);  // 1 метр вправо ОТНОСИТЕЛЬНО body
arm.child_of("body");
```

В мировом пространстве `arm` будет на позиции `(1.0, 1.0, 0.0)` — позиция parent + своя. При повороте `body` — `arm` поворачивается вместе с ним.

### Мировая матрица

Для каждого объекта вычисляется мировая матрица (world matrix):
- Для корневого объекта: `world = local_matrix`
- Для дочернего: `world = parent.world * local_matrix`

### Dirty flags

Оптимизация: мировая матрица пересчитывается **только при изменении** transform или rotate (своего или родительского). При изменении устанавливается dirty flag. Перед рендером — обход дерева, пересчёт только помеченных, сброс флагов.

Это важно для производительности: сцена с 10 000 объектов, где двигается один — пересчёт одного поддерева, а не всех 10 000 матриц.

---

## Сцена

Сцена описывается полностью в Rust-коде. Нет файлов сцен. Все объекты создаются через `game.world.spawn()` в startup-системе.

```rust
fn setup(game: &mut Engine) {
    // Земля
    let ground = game.world.spawn("ground");
    ground.transform.set(0.0, 0.0, 0.0);
    let ground_mesh = game.assets.load("models/ground.glb", AssetType::GLBModel);
    ground.mesh.set(ground_mesh);

    // Игрок
    let player = game.world.spawn("player");
    player.transform.set(0.0, 1.0, 0.0);
    let player_mesh = game.assets.load("models/character", AssetType::GLTFModel);
    player.mesh.set(player_mesh);
    player.script.set(Script::new(player_update));

    // Камера — дочерняя к игроку
    let cam = game.world.spawn("camera");
    cam.transform.set(0.0, 3.0, -5.0);
    cam.rotate.look_at_vec(Vec3::new(0.0, 1.0, 0.0));
    cam.camera.set(Camera { fov: 60.0, near: 0.1, far: 1000.0 });
    cam.child_of("player");  // следует за игроком

    // Солнце
    let sun = game.world.spawn("sun");
    sun.rotate.set(-0.5, 0.0, 0.0);
    sun.light.set(DirectionalLight { color: Color::DAYLIGHT, intensity: 1.0 });

    // Фоновый свет
    let ambient = game.world.spawn("ambient");
    ambient.light.set(AmbientLight { color: Color::COOL, intensity: 0.2 });
}
```

Это полная сцена. Нет JSON/TOML/XML файлов — всё в типизированном Rust-коде с проверкой компилятором.