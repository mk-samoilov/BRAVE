mod renderer;

pub use renderer::Renderer;
pub use brv_engine::RenderMode;

use brv_engine::{Engine, Plugin};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, game: &mut Engine) {
        let assets = game.assets.as_mut().expect("AssetPlugin not loaded before RenderPlugin");
        let window = game.window.as_ref().expect("WindowPlugin not loaded");
        let renderer = Renderer::new(window, assets);
        game.render = Some(Box::new(renderer));
    }
}
