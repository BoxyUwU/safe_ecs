use std::marker::PhantomData;

use crate::{Component, Entity, World};

pub trait Command: 'static {
    fn apply(self: Box<Self>, world: &mut World);
}

struct RemoveCmd<T: Component>(Entity, PhantomData<T>);
impl<T: Component> Command for RemoveCmd<T> {
    fn apply(self: Box<Self>, world: &mut World) {
        world.remove_component::<T>(self.0);
    }
}
struct InsertCmd<T: Component>(Entity, T);
impl<T: Component> Command for InsertCmd<T> {
    fn apply(self: Box<Self>, world: &mut World) {
        world.insert_component(self.0, self.1);
    }
}

pub struct CommandBuffer(Vec<Box<dyn Command>>);
impl CommandBuffer {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn apply(&mut self, world: &mut World) {
        world
            .entities
            .fix_reserved_entities(|reserved| world.archetypes[0].entities.push(reserved));
        for cmd in self.0.drain(..) {
            cmd.apply(world);
        }
    }
}
pub struct Commands<'a>(pub(crate) &'a mut CommandBuffer, pub(crate) &'a World);
pub struct CommandsWithEntity<'a, 'b>(&'a mut Commands<'b>, Entity);

impl<'a> Commands<'a> {
    pub fn entity(&mut self, entity: Entity) -> CommandsWithEntity<'_, 'a> {
        CommandsWithEntity(self, entity)
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> &mut Self {
        self.0
             .0
            .push(Box::new(RemoveCmd::<T>(entity, PhantomData)));
        self
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> &mut Self {
        self.0 .0.push(Box::new(InsertCmd::<T>(entity, component)));
        self
    }

    pub fn spawn(&mut self) -> CommandsWithEntity<'_, 'a> {
        let e = self.1.entities.reserve_entity();
        CommandsWithEntity(self, e)
    }
}

impl CommandsWithEntity<'_, '_> {
    pub fn remove<T: Component>(&mut self) -> &mut Self {
        self.0.remove_component::<T>(self.1);
        self
    }

    pub fn insert<T: Component>(&mut self, component: T) -> &mut Self {
        self.0.insert_component::<T>(self.1, component);
        self
    }

    pub fn id(&mut self) -> (Entity, &mut Self) {
        (self.1, self)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let e = world.spawn().id();
        // FIXME non 'static systems just for access scope lol
        world.access_scope(move |mut cmds: Commands| {
            cmds.entity(e).insert(10_u32).insert(12_u64).remove::<u32>();
        });
        let mut q = world.query::<&u32>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), None);
        let mut q = world.query::<&u64>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some(&12));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn spawn() {
        let mut world = World::new();
        // FIXME allow to return stuff..?
        world.access_scope(|mut cmds: Commands| {
            cmds.spawn().insert(10_u32).insert(12_u64).remove::<u32>();
        });

        let mut q = world.query::<&u32>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), None);
        let mut q = world.query::<&u64>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some(&12));
        assert_eq!(iter.next(), None);
    }
}
