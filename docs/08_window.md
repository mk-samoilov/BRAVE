# Модуль brv_window

Обёртка над `winit`. Одно окно. Подключается через `WindowPlugin`.

---

## WindowPlugin

```rust
game.add_plugin(WindowPlugin {
    title: "My Game",    // заголовок окна
    width: 1280,         // начальная ширина в пикселях
    height: 720,         // начальная высота в пикселях
});
```

WindowPlugin создаёт winit окно и сохраняет его в `game.window`. Должен быть добавлен **первым** среди плагинов — другие плагины (RenderPlugin) зависят от окна.

---

## API

```rust
// Чтение
game.window.width() -> u32              // текущая ширина окна
game.window.height() -> u32             // текущая высота окна

// Управление
game.window.quit()                       // закрыть окно и завершить приложение
game.window.set_title(title: &str)       // изменить заголовок
game.window.set_fullscreen(enabled: bool)// переключить полноэкранный режим
game.window.set_cursor_visible(visible: bool)   // показать/скрыть курсор
game.window.set_cursor_grabbed(grabbed: bool)   // захватить курсор в окне (для FPS камеры)
```

### set_cursor_grabbed

Когда `grabbed = true`:
- Курсор заблокирован в центре окна
- Курсор невидим
- `mouse_delta()` возвращает относительное движение мыши

Используется для FPS-камер, где мышь управляет поворотом камеры а не UI-курсором.

### set_fullscreen

Переключает между оконным и полноэкранным (borderless fullscreen) режимом. Swapchain пересоздаётся автоматически.

---

## Обработка событий

Window внутри хранит winit `Window` и обрабатывает:
- `WindowEvent::Resized` → пересоздание swapchain, обновление width/height
- `WindowEvent::CloseRequested` → выход из приложения
- Клавиатурные и мышиные события → пробрасываются в `brv_input`

Event loop принадлежит окну. `game.run()` вызывает `event_loop.run(...)` который забирает управление.