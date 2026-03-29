mod game;
mod camera;

use brv_core::prelude::*;

fn main() {
    let mut game = Engine::new();

    game.add_plugin(WindowPlugin { title: "My BRAVE Game", width: 1920, height: 1080 });
    game.add_plugin(InputPlugin);
    game.add_plugin(AssetPlugin::new("assets/"));
    game.add_plugin(RenderPlugin);

    game.add_startup_system(game::setup);
    game.add_system(game::update);

    game.run();
}
