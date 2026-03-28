# Модуль brv_input

Polling API для клавиатуры и мыши. Подключается через `InputPlugin`.

---

## Клавиатура

```rust
game.input.is_key_pressed(Key::Space)    // нажата именно в этом кадре (true один кадр)
game.input.is_key_held(Key::W)           // зажата (true пока держишь)
game.input.is_key_released(Key::Escape)  // отпущена именно в этом кадре (true один кадр)
```

### Разница pressed / held / released

- `is_key_pressed` — возвращает `true` только в тот кадр, когда клавиша была нажата. В следующем кадре = `false`, даже если клавиша всё ещё зажата. Используй для одноразовых действий: прыжок, выстрел, открытие меню.
- `is_key_held` — возвращает `true` всё время пока клавиша зажата. Используй для непрерывных действий: ходьба, поворот камеры.
- `is_key_released` — возвращает `true` только в тот кадр, когда клавиша была отпущена. Используй для действий при отпускании: отмена прицеливания.

### Реализация

Внутри — два массива состояний клавиш: `current_frame` и `previous_frame`. Каждый кадр перед вызовом систем:

1. `previous_frame = current_frame.clone()`
2. Обрабатываются winit события, обновляется `current_frame`
3. `pressed` = текущий `true`, предыдущий `false`
4. `released` = текущий `false`, предыдущий `true`
5. `held` = текущий `true`

---

## Мышь

```rust
game.input.mouse_position() -> (f32, f32)    // абсолютная позиция курсора (x, y) в пикселях
game.input.mouse_delta() -> (f32, f32)       // движение за кадр (dx, dy)
game.input.mouse_scroll() -> f32             // колёсико: положительное = вверх, отрицательное = вниз, 0.0 = не крутили

game.input.is_mouse_pressed(MouseButton::Left)    // нажата в этом кадре
game.input.is_mouse_held(MouseButton::Right)       // зажата
game.input.is_mouse_released(MouseButton::Middle)  // отпущена в этом кадре
```

### mouse_delta

Возвращает разницу позиции мыши с прошлого кадра. Полезно для FPS-камеры:

```rust
fn camera_look(obj: &mut Object, game: &mut Engine) {
    let (dx, dy) = game.input.as_ref().unwrap().mouse_delta();
    let rot = obj.rotate.get();
    let sensitivity = 0.003;
    obj.rotate.set(rot.x + dy * sensitivity, rot.y + dx * sensitivity, 0.0);
}
```

### mouse_scroll

Возвращает `f32` — количество "щелчков" колёсика за текущий кадр. Обычно ±1.0 за щелчок, но может быть дробным на тачпадах.

```rust
let scroll = game.input.as_ref().unwrap().mouse_scroll();
if scroll > 0.0 {
    // zoom in
} else if scroll < 0.0 {
    // zoom out
}
```

---

## Интеграция с winit

События клавиатуры и мыши приходят из winit event loop (в `WindowEvent`). Input обновляется каждый кадр **перед** вызовом систем и скриптов — это гарантирует что все системы в одном кадре видят одинаковое состояние ввода.