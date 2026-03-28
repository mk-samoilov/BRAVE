use brave_core::prelude::*;

mod game;
mod level;
mod player;
mod utils;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("calloop", log::LevelFilter::Error)
        .init();

    let mut game: Engine = Engine::new();

    game.add_plugin(WindowPlugin { title: "BRAVE Game Example", width: 1920, height: 1080 });
    game.add_plugin(InputPlugin);
    game.add_plugin(RenderPlugin);
    game.add_plugin(AssetPlugin::new("assets/"));

    game.add_startup_system(game::setup);
    game.add_system(player::player_look);
    game.add_system(game::update);

    run_brave(game);
}
