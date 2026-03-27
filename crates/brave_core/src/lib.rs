pub mod engine;
pub mod plugin;
pub mod runner;
pub mod time;

pub use engine::{
    AssetPlugin, Engine, InputPlugin, RenderPlugin, Script, ScriptFn, SystemFn, WindowPlugin,
};
pub use plugin::Plugin;
pub use runner::run_brave;
pub use time::Time;

pub mod prelude {
    pub use crate::engine::{AssetPlugin, Engine, InputPlugin, RenderPlugin, Script, WindowPlugin};
    pub use crate::plugin::Plugin;
    pub use crate::runner::run_brave;
    pub use crate::time::Time;
    pub use brave_assets::{Asset, AssetFormat, AssetManager};
    pub use brave_ecs::{Children, Component, Entity, Parent, Transform, World};
    pub use brave_scene::{descendants, detach, world_transform};
    pub use brave_input::{Key, MouseButton};
    pub use brave_math::{Color, Mat4, Quat, Vec2, Vec3, Vec4};
    pub use brave_render::{AmbientLight, Camera, DirectionalLight, Mesh, MeshRenderer,
                           PointLight, SpotLight, Vertex};
    pub use brave_base_meshes as shapes;
}
