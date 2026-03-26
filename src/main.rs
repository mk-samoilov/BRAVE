use brave_core::prelude::*;

mod game;
mod level;
mod player;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut game: Engine = Engine::new();

    game.add_plugin(WindowPlugin { title: "BRAVE Game Example", width: 1280, height: 720 });
    game.add_plugin(InputPlugin);
    game.add_plugin(RenderPlugin);
    game.add_plugin(AssetPlugin::new("assets/"));

    game.add_startup_system(game::setup);
    game.add_system(game::update);

    run_brave(game);
}
