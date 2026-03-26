use crate::engine::Engine;

pub trait Plugin: 'static {
    fn build(&self, engine: &mut Engine);
}
