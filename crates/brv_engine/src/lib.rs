mod time;
mod object;
mod world;
mod types;

pub use time::Time;
pub use object::{
    Object, TransformField, RotateField, VisibleField,
    Script, Transform, Component, OptionField,
};
pub use world::World;
pub use types::{
    Camera, MeshComponent, MeshData, Vertex, Light,
    DirectionalLight, PointLight, SpotLight, AmbientLight,
};

use std::any::TypeId;
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

pub trait Plugin: 'static {
    fn build(&self, game: &mut Engine);
}

pub trait RenderBackend: 'static {
    fn draw_frame(&mut self, world: &World);
    fn on_resize(&mut self, width: u32, height: u32);
}

pub enum RenderMode {
    Rasterization,
    PathTracing,
}

pub struct Engine {
    pub time: Time,
    pub world: World,
    pub window: Option<brv_window::Window>,
    pub input: Option<brv_input::Input>,
    pub render: Option<Box<dyn RenderBackend>>,
    systems: Vec<fn(&mut Engine)>,
    startup_systems: Vec<fn(&mut Engine)>,
    registered_plugins: Vec<TypeId>,
}

impl Engine {
    pub fn new() -> Self {
        env_logger::Builder::from_env(
            env_logger::Env::default().filter_or("BRAVE_LOG", "info")
        ).init();
        Self {
            time: Time::new(),
            world: World::new(),
            window: None,
            input: None,
            render: None,
            systems: Vec::new(),
            startup_systems: Vec::new(),
            registered_plugins: Vec::new(),
        }
    }

    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) {
        let id = TypeId::of::<P>();
        if self.registered_plugins.contains(&id) {
            return;
        }
        self.registered_plugins.push(id);
        plugin.build(self);
    }

    pub fn add_startup_system(&mut self, system: fn(&mut Engine)) {
        self.startup_systems.push(system);
    }

    pub fn add_system(&mut self, system: fn(&mut Engine)) {
        self.systems.push(system);
    }

    pub fn remove_system(&mut self, system: fn(&mut Engine)) {
        self.systems.retain(|&s| !std::ptr::fn_addr_eq(s, system));
    }

    pub fn run(mut self) {
        let startup = std::mem::take(&mut self.startup_systems);
        for system in startup {
            system(&mut self);
        }

        let event_loop = self.window
            .as_mut()
            .expect("WindowPlugin not loaded")
            .take_event_loop();

        let mut accumulator = 0.0f32;

        let _ = event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { ref event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                            return;
                        }
                        WindowEvent::Resized(size) => {
                            if let Some(render) = self.render.as_mut() {
                                render.on_resize(size.width, size.height);
                            }
                        }
                        _ => {}
                    }
                    if let Some(input) = self.input.as_mut() {
                        input.handle_window_event(event);
                    }
                }
                Event::DeviceEvent { ref event, .. } => {
                    if let Some(input) = self.input.as_mut() {
                        input.handle_device_event(event);
                    }
                }
                Event::AboutToWait => {
                    let dt = self.time.tick();
                    accumulator += dt;

                    let fixed_dt = self.time.fixed_delta();
                    while accumulator >= fixed_dt {
                        let names = self.world.script_names();
                        for name in names {
                            if let Some(obj_ptr) = self.world.get_script_ptr(&name) {
                                let func = unsafe { (*obj_ptr).script.as_ref() }
                                    .map(|s| s.func);
                                if let Some(func) = func {
                                    let obj = unsafe { &mut *obj_ptr };
                                    func(obj, &mut self);
                                }
                            }
                        }
                        accumulator -= fixed_dt;
                    }

                    let systems = self.systems.clone();
                    for system in systems {
                        system(&mut self);
                    }

                    if let Some(render) = self.render.as_mut() {
                        let world_ptr: *const World = &self.world;
                        render.draw_frame(unsafe { &*world_ptr });
                    }

                    if self.window.as_ref().map_or(false, |w| w.should_quit()) {
                        elwt.exit();
                        return;
                    }

                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }

                    if let Some(input) = self.input.as_mut() {
                        input.begin_frame();
                    }

                    let frame_time = std::time::Duration::from_secs_f32(1.0 / self.time.target_fps());
                    elwt.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + frame_time,
                    ));
                }
                _ => {}
            }
        });
    }
}

pub struct WindowPlugin {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

impl Plugin for WindowPlugin {
    fn build(&self, game: &mut Engine) {
        game.window = Some(brv_window::Window::new(brv_window::WindowConfig {
            title: self.title,
            width: self.width,
            height: self.height,
        }));
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, game: &mut Engine) {
        game.input = Some(brv_input::Input::new());
    }
}
