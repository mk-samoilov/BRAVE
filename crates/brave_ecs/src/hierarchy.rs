use crate::component::Component;

pub struct Parent {
    pub name: String,
}

impl Parent {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Component for Parent {}

pub struct Children {
    pub names: Vec<String>,
}

impl Children {
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }

    pub fn add(&mut self, name: impl Into<String>) {
        self.names.push(name.into());
    }

    pub fn remove(&mut self, name: &str) {
        self.names.retain(|n| n != name);
    }
}

impl Default for Children {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Children {}
