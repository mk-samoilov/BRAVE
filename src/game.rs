use brv_core::prelude::*;

pub fn setup(game: &mut Engine) {
    let player = game.world.spawn("player");
    player.transform.set(0.0, 1.0, 0.0);
    player.script.set(Script::new(crate::player::update));
}

pub fn update(game: &mut Engine) {
    if let Some(input) = &game.input {
        if input.is_key_pressed(Key::Escape) {
            game.window.as_ref().unwrap().quit();
        }
    }
}
