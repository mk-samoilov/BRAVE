mod game;
mod player;

use brv_core::prelude::*;

fn main() {
    let mut engine = Engine::new();

    engine.add_plugin(WindowPlugin { title: "BRAVE", width: 1280, height: 720 });
    engine.add_plugin(InputPlugin);

    engine.add_startup_system(game::setup);
    engine.add_system(game::update);

    engine.run();
}
