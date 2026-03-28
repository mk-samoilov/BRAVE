use brv_math::{Vec3, Quat};
use glam::EulerRot;
use crate::types::{Camera, MeshComponent, Light};

pub trait OptionField<T> {
    fn set<V: Into<T>>(&mut self, value: V);
    fn clear(&mut self);
}

impl<T> OptionField<T> for Option<T> {
    fn set<V: Into<T>>(&mut self, value: V) {
        *self = Some(value.into());
    }
    fn clear(&mut self) {
        *self = None;
    }
}

pub struct TransformField {
    position: Vec3,
    scale: Vec3,
}

impl TransformField {
    pub(crate) fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }

    pub fn set(&mut self, x: f32, y: f32, z: f32) {
        self.position = Vec3::new(x, y, z);
    }

    pub fn get(&self) -> Vec3 {
        self.position
    }

    pub fn set_scale(&mut self, x: f32, y: f32, z: f32) {
        self.scale = Vec3::new(x, y, z);
    }

    pub fn get_scale(&self) -> Vec3 {
        self.scale
    }
}

pub struct RotateField {
    quat: Quat,
}

impl RotateField {
    pub(crate) fn new() -> Self {
        Self { quat: Quat::IDENTITY }
    }

    pub fn set(&mut self, x: f32, y: f32, z: f32) {
        self.quat = Quat::from_euler(EulerRot::XYZ, x, y, z);
    }

    pub fn get(&self) -> Vec3 {
        let (x, y, z) = self.quat.to_euler(EulerRot::XYZ);
        Vec3::new(x, y, z)
    }

    pub fn quat(&self) -> Quat {
        self.quat
    }
}

pub struct VisibleField(bool);

impl VisibleField {
    pub(crate) fn new() -> Self {
        Self(true)
    }

    pub fn set(&mut self, visible: bool) {
        self.0 = visible;
    }

    pub fn get(&self) -> bool {
        self.0
    }
}

pub struct Script {
    pub(crate) func: fn(&mut Object, &mut crate::Engine),
}

impl Script {
    pub fn new(func: fn(&mut Object, &mut crate::Engine)) -> Self {
        Self { func }
    }
}

pub struct Transform {
    x: f32,
    y: f32,
    z: f32,
}

impl Transform {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

pub struct Object {
    pub transform: TransformField,
    pub rotate: RotateField,
    pub visible: VisibleField,
    pub mesh: Option<MeshComponent>,
    pub camera: Option<Camera>,
    pub light: Option<Light>,
    pub script: Option<Script>,
}

impl Object {
    pub(crate) fn new() -> Self {
        Self {
            transform: TransformField::new(),
            rotate: RotateField::new(),
            visible: VisibleField::new(),
            mesh: None,
            camera: None,
            light: None,
            script: None,
        }
    }

    pub fn with_transform(mut self, x: f32, y: f32, z: f32) -> Self {
        self.transform.set(x, y, z);
        self
    }

    pub fn look_at_obj(&mut self, other: &Object) {
        let from = self.transform.get();
        let target = other.transform.get();
        let dir = (target - from).normalize_or_zero();
        if dir.length_squared() > f32::EPSILON {
            self.rotate.quat = Quat::from_rotation_arc(Vec3::Z, dir);
        }
    }

    pub fn look_at_vec(&mut self, target: Vec3) {
        let from = self.transform.get();
        let dir = (target - from).normalize_or_zero();
        if dir.length_squared() > f32::EPSILON {
            self.rotate.quat = Quat::from_rotation_arc(Vec3::Z, dir);
        }
    }

    pub fn with<C: Component>(&mut self, component: C) -> &mut Self {
        component.apply(self);
        self
    }
}

pub trait Component {
    fn apply(self, obj: &mut Object);
}

impl Component for Transform {
    fn apply(self, obj: &mut Object) {
        obj.transform.set(self.x, self.y, self.z);
    }
}

impl Component for Camera {
    fn apply(self, obj: &mut Object) {
        obj.camera = Some(self);
    }
}

impl Component for Script {
    fn apply(self, obj: &mut Object) {
        obj.script = Some(self);
    }
}
