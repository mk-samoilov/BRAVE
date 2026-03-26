use brave_core::prelude::*;

pub fn player_update(entity: &mut Entity, game: &mut Engine) {
    let tr = entity.get_mut::<Transform>();
    let speed = 5.0 * game.time.fixed_delta();

    if game.input().is_key_held(Key::KeyW) { tr.position.z -= speed; }
    if game.input().is_key_held(Key::KeyS) { tr.position.z += speed; }
    if game.input().is_key_held(Key::KeyA) { tr.position.x -= speed; }
    if game.input().is_key_held(Key::KeyD) { tr.position.x += speed; }
    if game.input().is_key_pressed(Key::Space) { tr.position.y += 2.0; }
}
