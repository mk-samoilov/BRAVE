mod renderer;

pub use renderer::Renderer;
pub use brv_engine::RenderMode;

use brv_engine::{Engine, Plugin};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, game: &mut Engine) {
        let window = game.window.as_ref().expect("WindowPlugin not loaded");
        let renderer = Renderer::new(window);
        game.render = Some(Box::new(renderer));
    }
}
