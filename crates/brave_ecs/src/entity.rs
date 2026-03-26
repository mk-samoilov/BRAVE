use std::any::TypeId;
use std::collections::HashMap;

use crate::component::Component;

pub struct Entity {
    pub name: String,
    components: HashMap<TypeId, Box<dyn std::any::Any>>,
}

impl Entity {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), components: HashMap::new() }
    }

    pub fn get<T: Component>(&self) -> &T {
        self.components
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("Component {} not found on '{}'", std::any::type_name::<T>(), self.name))
    }

    pub fn get_mut<T: Component>(&mut self) -> &mut T {
        let name = &self.name;
        self.components
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
            .unwrap_or_else(|| panic!("Component {} not found on '{}'", std::any::type_name::<T>(), name))
    }

    pub fn try_get<T: Component>(&self) -> Option<&T> {
        self.components.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref::<T>())
    }

    pub fn try_get_mut<T: Component>(&mut self) -> Option<&mut T> {
        self.components.get_mut(&TypeId::of::<T>()).and_then(|b| b.downcast_mut::<T>())
    }

    pub fn has<T: Component>(&self) -> bool {
        self.components.contains_key(&TypeId::of::<T>())
    }

    pub fn add<T: Component>(&mut self, component: T) {
        self.components.insert(TypeId::of::<T>(), Box::new(component));
    }

    pub fn remove<T: Component>(&mut self) {
        self.components.remove(&TypeId::of::<T>());
    }
}
