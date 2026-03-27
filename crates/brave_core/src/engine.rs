use std::any::TypeId;
use std::collections::HashSet;

use brave_assets::AssetManager;
use brave_ecs::{Component, Entity, World};
use brave_input::Input;
use brave_render::{MeshRenderer, Renderer};
use brave_scene::world_transform;
use brave_window::{ActiveEventLoop, Window, WindowConfig};

use crate::plugin::Plugin;
use crate::time::{Time, FIXED_DT};

pub type SystemFn = fn(&mut Engine);

pub type ScriptFn = fn(&mut Entity, &mut Engine);

pub struct Script {
    pub func: ScriptFn,
}

impl Script {
    pub fn new(func: ScriptFn) -> Self {
        Self { func }
    }
}

impl Component for Script {}

pub struct Engine {
    pub world: World,
    pub window: Option<Window>,
    pub input: Option<Input>,
    pub render: Option<Renderer>,
    pub assets: Option<AssetManager>,
    pub time: Time,
    systems: Vec<SystemFn>,
    startup_systems: Vec<SystemFn>,
    registered_plugins: HashSet<TypeId>,
    pending_window_config: Option<WindowConfig>,
    render_pending: bool,
    pub(crate) accumulator: f32,
    pub(crate) startup_done: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            window: None,
            input: None,
            render: None,
            assets: None,
            time: Time::new(),
            systems: Vec::new(),
            startup_systems: Vec::new(),
            registered_plugins: HashSet::new(),
            pending_window_config: None,
            render_pending: false,
            accumulator: 0.0,
            startup_done: false,
        }
    }

    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) {
        let id = TypeId::of::<P>();
        assert!(
            self.registered_plugins.insert(id),
            "Plugin {} already registered",
            std::any::type_name::<P>()
        );
        plugin.build(self);
    }

    pub fn add_system(&mut self, f: SystemFn) {
        self.systems.push(f);
    }

    pub fn remove_system(&mut self, f: SystemFn) {
        self.systems.retain(|&s| s as usize != f as usize);
    }

    pub fn add_startup_system(&mut self, f: SystemFn) {
        self.startup_systems.push(f);
    }

    pub fn window(&self) -> &Window {
        self.window.as_ref().expect("WindowPlugin not loaded")
    }

    pub fn window_mut(&mut self) -> &mut Window {
        self.window.as_mut().expect("WindowPlugin not loaded")
    }

    pub fn input(&self) -> &Input {
        self.input.as_ref().expect("InputPlugin not loaded")
    }

    pub fn input_mut(&mut self) -> &mut Input {
        self.input.as_mut().expect("InputPlugin not loaded")
    }

    pub fn set_pending_window_config(&mut self, config: WindowConfig) {
        self.pending_window_config = Some(config);
    }

    pub(crate) fn create_window_if_pending(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(config) = self.pending_window_config.take() {
            self.window = Some(Window::new(event_loop, &config));
        }
    }

    pub(crate) fn create_renderer_if_pending(&mut self) {
        if self.render_pending {
            if let Some(window) = &self.window {
                let renderer = Renderer::new(window);
                if let Some(assets) = &mut self.assets {
                    unsafe {
                        assets.connect_renderer(
                            renderer.ctx() as *const _,
                            renderer.command_pool(),
                        );
                    }
                }
                self.render = Some(renderer);
                self.render_pending = false;
                log::info!("Renderer created");
            }
        }
    }

    pub fn render(&self) -> &Renderer {
        self.render.as_ref().expect("RenderPlugin not loaded")
    }

    pub fn render_mut(&mut self) -> &mut Renderer {
        self.render.as_mut().expect("RenderPlugin not loaded")
    }

    pub fn assets(&self) -> &AssetManager {
        self.assets.as_ref().expect("AssetPlugin not loaded")
    }

    pub fn assets_mut(&mut self) -> &mut AssetManager {
        self.assets.as_mut().expect("AssetPlugin not loaded")
    }

    pub(crate) fn run_startup(&mut self) {
        let systems: Vec<SystemFn> = self.startup_systems.drain(..).collect();
        for system in systems {
            system(self);
        }
        self.startup_done = true;
    }

    pub(crate) fn run_frame(&mut self) {
        if let Some(input) = &mut self.input {
            input.begin_frame();
        }

        let dt = self.time.tick();
        self.accumulator += dt;

        while self.accumulator >= FIXED_DT {
            self.run_fixed_update();
            self.accumulator -= FIXED_DT;
        }

        let systems: Vec<SystemFn> = self.systems.clone();
        for system in systems {
            system(self);
        }

        self.time.throttle();
    }

    fn run_fixed_update(&mut self) {
        let names = self.world.names_with::<Script>();

        for name in names {
            let mut entity = match self.world.take(&name) {
                Some(e) => e,
                None => continue,
            };

            let func = entity.get::<Script>().func;
            func(&mut entity, self);

            self.world.put_back(entity);
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WindowPlugin {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

impl Plugin for WindowPlugin {
    fn build(&self, engine: &mut Engine) {
        engine.set_pending_window_config(WindowConfig {
            title: self.title,
            width: self.width,
            height: self.height,
        });
        log::info!("WindowPlugin registered");
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, engine: &mut Engine) {
        engine.input = Some(Input::new());
        log::info!("InputPlugin loaded");
    }
}

pub struct AssetPlugin {
    pub path: &'static str,
}

impl AssetPlugin {
    pub fn new(path: &'static str) -> Self {
        Self { path }
    }
}

impl Plugin for AssetPlugin {
    fn build(&self, engine: &mut Engine) {
        engine.assets = Some(AssetManager::new(self.path));
        log::info!("AssetPlugin loaded (path: {})", self.path);
    }
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, engine: &mut Engine) {
        engine.render_pending = true;
        engine.add_system(render_system);
        log::info!("RenderPlugin registered");
    }
}

fn render_system(engine: &mut Engine) {
    let (width, height) = match &engine.window {
        Some(w) => (w.width(), w.height()),
        None => return,
    };

    let world_transforms: std::collections::HashMap<String, brave_math::Mat4> = engine
        .world
        .entities()
        .filter(|e| e.has::<MeshRenderer>())
        .map(|e| (e.name.clone(), world_transform(&e.name, &engine.world)))
        .collect();

    if let Some(renderer) = &mut engine.render {
        renderer.render_frame(&engine.world, &world_transforms, width, height);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(render) = &self.render {
            render.wait_idle();
        }
    }
}
