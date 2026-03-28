use std::collections::HashMap;
use crate::object::Object;

pub struct World {
    objects: HashMap<String, Box<Object>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, name: &str) -> &mut Object {
        if self.objects.contains_key(name) {
            panic!("Object '{}' already exists", name);
        }
        self.objects.insert(name.to_string(), Box::new(Object::new()));
        self.objects.get_mut(name).unwrap().as_mut()
    }

    pub fn get_obj(&self, name: &str) -> Option<&Object> {
        self.objects.get(name).map(|b| b.as_ref())
    }

    pub fn get_obj_mut(&mut self, name: &str) -> Option<&mut Object> {
        self.objects.get_mut(name).map(|b| b.as_mut())
    }

    pub fn remove_obj(&mut self, name: &str) {
        self.objects.remove(name);
    }

    pub(crate) fn script_names(&self) -> Vec<String> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.script.is_some())
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub(crate) fn get_script_ptr(&mut self, name: &str) -> Option<*mut Object> {
        self.objects
            .get_mut(name)
            .map(|b| b.as_mut() as *mut Object)
    }
}
