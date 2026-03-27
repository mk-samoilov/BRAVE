use brave_ecs::{Children, Parent, Transform, World};
use brave_math::Mat4;

pub fn world_transform(name: &str, world: &World) -> Mat4 {
    let entity = world.get(name);

    let local = entity
        .try_get::<Transform>()
        .map(|t| t.matrix())
        .unwrap_or(Mat4::IDENTITY);

    if let Some(parent) = entity.try_get::<Parent>() {
        let parent_name = parent.name.clone();
        let parent_world = world_transform(&parent_name, world);
        parent_world * local
    } else {
        local
    }
}

pub fn descendants(name: &str, world: &World) -> Vec<String> {
    let entity = world.get(name);
    let mut result = Vec::new();

    if let Some(children) = entity.try_get::<Children>() {
        for child_name in children.names.clone() {
            result.push(child_name.clone());
            result.extend(descendants(&child_name, world));
        }
    }
    result
}

pub fn detach(name: &str, world: &mut World) {
    let parent_name = world
        .get(name)
        .try_get::<Parent>()
        .map(|p| p.name.clone());

    if let Some(parent_name) = parent_name {
        world.get_mut(name).remove::<Parent>();
        if world.exists(&parent_name)
            && let Some(children) = world.get_mut(&parent_name).try_get_mut::<Children>()
        {
            children.remove(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brave_ecs::World;

    #[test]
    fn child_of_sets_parent() {
        let mut world = World::new();
        world.spawn("player").with(Transform::new(1.0, 0.0, 0.0));
        world.spawn("arm")
            .with(Transform::new(0.5, 0.0, 0.0))
            .child_of("player");

        assert!(world.get("arm").has::<Parent>());
        assert_eq!(world.get("arm").get::<Parent>().name, "player");
        assert!(world.get("player").has::<Children>());
        assert!(world.get("player").get::<Children>().names.contains(&"arm".to_string()));
    }

    #[test]
    fn world_transform_accumulates() {
        let mut world = World::new();
        world.spawn("root").with(Transform::new(10.0, 0.0, 0.0));
        world.spawn("child")
            .with(Transform::new(5.0, 0.0, 0.0))
            .child_of("root");

        let mat = world_transform("child", &world);
        let pos = mat.col(3);
        assert!((pos.x - 15.0).abs() < 0.001, "x = {}", pos.x);
    }

    #[test]
    fn world_transform_no_parent() {
        let mut world = World::new();
        world.spawn("solo").with(Transform::new(3.0, 4.0, 5.0));
        let mat = world_transform("solo", &world);
        let pos = mat.col(3);
        assert!((pos.x - 3.0).abs() < 0.001);
        assert!((pos.y - 4.0).abs() < 0.001);
    }

    #[test]
    fn detach_removes_parent_and_child() {
        let mut world = World::new();
        world.spawn("root").with(Transform::new(0.0, 0.0, 0.0));
        world.spawn("leaf")
            .with(Transform::new(1.0, 0.0, 0.0))
            .child_of("root");

        detach("leaf", &mut world);

        assert!(!world.get("leaf").has::<Parent>());
        assert!(world.get("root").get::<Children>().names.is_empty());
    }

    #[test]
    fn descendants_recursive() {
        let mut world = World::new();
        world.spawn("a").with(Transform::new(0.0, 0.0, 0.0));
        world.spawn("b").with(Transform::new(1.0, 0.0, 0.0)).child_of("a");
        world.spawn("c").with(Transform::new(2.0, 0.0, 0.0)).child_of("b");

        let d = descendants("a", &world);
        assert_eq!(d.len(), 2);
        assert!(d.contains(&"b".to_string()));
        assert!(d.contains(&"c".to_string()));
    }
}
