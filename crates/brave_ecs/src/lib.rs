pub mod component;
pub mod entity;
pub mod hierarchy;
pub mod transform;
pub mod world;

pub use component::Component;
pub use entity::Entity;
pub use hierarchy::{Children, Parent};
pub use transform::Transform;
pub use world::{EntityBuilder, World};

#[cfg(test)]
mod tests {
    use super::*;

    struct Health(i32);
    impl Component for Health {}

    struct Speed(f32);
    impl Component for Speed {}

    #[test]
    fn spawn_and_get() {
        let mut world = World::new();
        world.spawn("player").with(Health(100)).with(Speed(5.0));

        let e = world.get("player");
        assert_eq!(e.get::<Health>().0, 100);
        assert_eq!(e.get::<Speed>().0, 5.0);
        assert!(e.has::<Health>());
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        world.spawn("enemy").with(Health(50));
        assert!(world.exists("enemy"));
        world.despawn("enemy");
        assert!(!world.exists("enemy"));
    }

    #[test]
    fn get_mut() {
        let mut world = World::new();
        world.spawn("player").with(Health(100));
        world.get_mut("player").get_mut::<Health>().0 -= 30;
        assert_eq!(world.get("player").get::<Health>().0, 70);
    }

    #[test]
    fn remove_component() {
        let mut world = World::new();
        world.spawn("box").with(Health(10)).with(Speed(1.0));
        world.get_mut("box").remove::<Speed>();
        assert!(!world.get("box").has::<Speed>());
        assert!(world.get("box").has::<Health>());
    }

    #[test]
    fn names_with() {
        let mut world = World::new();
        world.spawn("a").with(Health(1));
        world.spawn("b").with(Speed(2.0));
        world.spawn("c").with(Health(3)).with(Speed(0.5));

        let names = world.names_with::<Health>();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"c".to_string()));
    }

    #[test]
    fn take_and_put_back() {
        let mut world = World::new();
        world.spawn("x").with(Health(42));

        let entity = world.take("x").unwrap();
        assert!(!world.exists("x"));
        world.put_back(entity);
        assert!(world.exists("x"));
        assert_eq!(world.get("x").get::<Health>().0, 42);
    }
}
