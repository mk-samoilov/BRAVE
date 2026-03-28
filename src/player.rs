use brv_core::prelude::*;

pub fn update(obj: &mut Object, game: &mut Engine) {
    let speed = 5.0 * game.time.fixed_delta();
    let pos = obj.transform.get();

    if let Some(input) = game.input.as_ref() {
        if input.is_key_held(Key::W) {
            obj.transform.set(pos.x, pos.y, pos.z + speed);
        }
        if input.is_key_held(Key::S) {
            obj.transform.set(pos.x, pos.y, pos.z - speed);
        }
        if input.is_key_held(Key::A) {
            obj.transform.set(pos.x - speed, pos.y, pos.z);
        }
        if input.is_key_held(Key::D) {
            obj.transform.set(pos.x + speed, pos.y, pos.z);
        }
    }
}
