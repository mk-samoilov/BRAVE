use std::collections::HashMap;

use crate::component::Component;
use crate::entity::Entity;
use crate::hierarchy::{Children, Parent};

pub struct World {
    entities: HashMap<String, Entity>,
    order: Vec<String>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn spawn(&mut self, name: &str) -> EntityBuilder<'_> {
        let entity = Entity::new(name);
        self.entities.insert(name.to_string(), entity);
        self.order.push(name.to_string());
        EntityBuilder { world: self, name: name.to_string() }
    }

    pub fn despawn(&mut self, name: &str) {
        self.entities.remove(name);
        self.order.retain(|n| n != name);
    }

    pub fn get(&self, name: &str) -> &Entity {
        self.entities.get(name)
            .unwrap_or_else(|| panic!("Entity '{}' not found", name))
    }

    pub fn get_mut(&mut self, name: &str) -> &mut Entity {
        self.entities.get_mut(name)
            .unwrap_or_else(|| panic!("Entity '{}' not found", name))
    }

    pub fn exists(&self, name: &str) -> bool {
        self.entities.contains_key(name)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.order.iter().filter_map(|n| self.entities.get(n))
    }

    pub fn entities_mut(&mut self) -> impl Iterator<Item = &mut Entity> {
        self.entities.values_mut()
    }

    pub fn take(&mut self, name: &str) -> Option<Entity> {
        self.entities.remove(name)
    }

    pub fn put_back(&mut self, entity: Entity) {
        self.entities.insert(entity.name.clone(), entity);
    }

    pub fn names_with<T: Component>(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|n| self.entities.get(*n).is_some_and(|e| e.has::<T>()))
            .cloned()
            .collect()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EntityBuilder<'w> {
    world: &'w mut World,
    pub name: String,
}

impl<'w> EntityBuilder<'w> {
    pub fn with<T: Component>(self, component: T) -> Self {
        self.world.entities.get_mut(&self.name)
            .expect("Entity disappeared during builder chain")
            .add(component);
        self
    }

    pub fn child_of(self, parent_name: &str) -> Self {
        self.world
            .entities
            .get_mut(&self.name)
            .expect("Entity disappeared during child_of")
            .add(Parent::new(parent_name));

        if let Some(parent_entity) = self.world.entities.get_mut(parent_name) {
            if parent_entity.has::<Children>() {
                parent_entity.get_mut::<Children>().add(&self.name);
            } else {
                let mut children = Children::new();
                children.add(&self.name);
                parent_entity.add(children);
            }
        }

        self
    }
}
