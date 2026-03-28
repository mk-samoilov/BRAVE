# Модуль brv_colors

Структура `Color` и набор пресетов. Отдельный крейт — цвета используются и в рендере (свет, материалы), и потенциально в будущем UI.

---

## Color struct

```rust
pub struct Color {
    pub r: f32,    // красный, 0.0..1.0
    pub g: f32,    // зелёный, 0.0..1.0
    pub b: f32,    // синий, 0.0..1.0
    pub a: f32,    // альфа (прозрачность), 0.0..1.0
}
```

### Конструктор

```rust
Color::new(r: f32, g: f32, b: f32, a: f32) -> Color
```

---

## Базовые пресеты

```rust
Color::WHITE     // (1.0, 1.0, 1.0, 1.0) — белый
Color::BLACK     // (0.0, 0.0, 0.0, 1.0) — чёрный
Color::RED       // (1.0, 0.0, 0.0, 1.0) — красный
Color::GREEN     // (0.0, 1.0, 0.0, 1.0) — зелёный
Color::BLUE      // (0.0, 0.0, 1.0, 1.0) — синий
```

## Пресеты для освещения

Цвета, подобранные для типичных световых условий:

```rust
Color::WARM      // тёплый жёлтый — лампа накаливания, свеча, факел
Color::COOL      // холодный голубой — лунный свет, ночное освещение
Color::DAYLIGHT  // дневной свет — нейтральный белый с лёгкой желтизной
Color::SUNSET    // закат — тёплый оранжево-розовый
```

---

## Использование

```rust
// Свет
sun.light.set(DirectionalLight { color: Color::DAYLIGHT, intensity: 1.0 });
lamp.light.set(PointLight { color: Color::WARM, intensity: 5.0, range: 10.0 });

// Кастомный цвет
let neon_pink = Color::new(1.0, 0.0, 0.5, 1.0);
neon_light.light.set(PointLight { color: neon_pink, intensity: 3.0, range: 8.0 });
```