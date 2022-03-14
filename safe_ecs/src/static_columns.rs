use std::{
    cell::{Ref, RefCell, RefMut},
    rc::{Rc, Weak},
};

use crate::{
    world::{Columns, WorldId},
    Component, EcsTypeId, Entity, World,
};

pub struct StaticColumnsInner<T>(pub(crate) Vec<Vec<T>>);
impl<T> StaticColumnsInner<T> {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self(vec![vec![]])))
    }
}
pub struct StaticColumns<T> {
    pub(crate) inner: Rc<RefCell<StaticColumnsInner<T>>>,
    pub(crate) id: EcsTypeId,
    pub(crate) world_id: WorldId,
}
impl<T: Component> StaticColumns<T> {
    pub(crate) fn new(
        id: EcsTypeId,
        world_id: WorldId,
    ) -> (Self, Weak<RefCell<StaticColumnsInner<T>>>) {
        let columns = StaticColumns::<T> {
            inner: StaticColumnsInner::new(),
            id,
            world_id,
        };
        let weak_ref = Rc::downgrade(&columns.inner);
        (columns, weak_ref)
    }
}

impl<T: Component> StaticColumns<T> {
    fn get_column(&self, _: &World, idx: usize) -> Ref<'_, [T]> {
        Ref::map(self.inner.borrow(), |inner| &inner.0[idx][..])
    }

    fn get_column_mut(&mut self, _: &World, idx: usize) -> RefMut<'_, [T]> {
        RefMut::map(self.inner.borrow_mut(), |inner| &mut inner.0[idx][..])
    }

    pub(crate) fn assert_world_id(&self, id: WorldId) {
        if self.world_id != id {
            panic!(
                "[Mismatched WorldIds]: attempt to use World: WorldId({}) with StaticColumn<{}>: WorldId({})",
                self.world_id.inner(),
                core::any::type_name::<T>(),
                id.inner(),
            )
        }
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn ecs_type_id(&self) -> EcsTypeId {
        self.id
    }

    pub fn get_component(&self, world: &World, entity: Entity) -> Option<Ref<'_, T>> {
        self.assert_world_id(world.id);
        let archetype_id = world.entities.meta(entity)?.archetype;
        let archetype = &world.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = *archetype.column_indices.get(&self.id)?;
        Some(Ref::map(self.get_column(world, column_idx), |col| {
            &col[entity_idx]
        }))
    }

    pub fn get_component_mut(&mut self, world: &World, entity: Entity) -> Option<RefMut<'_, T>> {
        self.assert_world_id(world.id);
        let archetype_id = world.entities.meta(entity)?.archetype;
        let archetype = &world.archetypes[archetype_id];
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = *archetype.column_indices.get(&self.id)?;
        Some(RefMut::map(self.get_column_mut(world, column_idx), |col| {
            &mut col[entity_idx]
        }))
    }

    // strictly speaking this does not need `self` at all, this could be a method
    // on world taking `&self` as it stores a handle to this storage.
    pub fn has_component(&self, world: &World, entity: Entity) -> bool {
        self.assert_world_id(world.id);
        let archetype_id = match world.entities.meta(entity) {
            Some(meta) => meta.archetype,
            None => return false,
        };
        world.archetypes[archetype_id]
            .column_indices
            .contains_key(&self.id)
    }

    // strictly speaking this does not need `self` at all, this could be a method on
    // world as it stores a handle to this storage. Taking `&mut self` "feels" more correct tho.
    // this operation potentially involves mutably accessing every single storage tied to the world.
    pub fn insert_component(
        &mut self,
        world: &mut World,
        entity: Entity,
        component: T,
    ) -> Option<T> {
        self.assert_world_id(world.id);
        if let Some(mut old_component) = self.get_component_mut(world, entity) {
            return Some(core::mem::replace(&mut *old_component, component));
        }

        let new_archetype = world.move_entity_from_insert(entity, self.id)?;
        let column_idx = *new_archetype.column_indices.get(&self.id).unwrap();
        self.inner.borrow_mut().0[column_idx].push(component);
        None
    }

    /// same as `insert_component`
    pub fn remove_component(&mut self, world: &mut World, entity: Entity) -> Option<T> {
        self.assert_world_id(world.id);
        if self.has_component(&*world, entity) == false {
            return None;
        }
        let (entity_idx, old_archetype) = world.move_entity_from_remove(entity, self.id)?;
        let column_idx = *old_archetype.column_indices.get(&self.id).unwrap();
        Some(self.inner.borrow_mut().0[column_idx].swap_remove(entity_idx))
    }
}

impl<T: Component> Columns for StaticColumnsInner<T> {
    fn push_empty_column(&mut self) {
        self.0.push(vec![]);
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn swap_remove_to(&mut self, old_col: usize, new_col: usize, entity_idx: usize) {
        let cols = &mut self.0[..];
        let (old_col, end_col) = crate::get_two_mut(cols, old_col, new_col);
        end_col.push(old_col.swap_remove(entity_idx));
    }

    fn swap_remove_drop(&mut self, col: usize, entity_idx: usize) {
        let col = &mut self.0[col];
        col.swap_remove(entity_idx);
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    trait UnwrapNone {
        fn unwrap_none(self);
    }

    impl<T> UnwrapNone for Option<T> {
        fn unwrap_none(self) {
            match self {
                Some(_) => panic!("expected `None` found `Some(_)`"),
                None => (),
            }
        }
    }

    #[test]
    fn has_component() {
        let mut world = World::new();
        let u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        assert_eq!(u32s.has_component(&world, e), false);
    }

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 10_u32);
    }

    #[test]
    fn insert_overwrite() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(
            u32s.insert_component(&mut world, e, 12_u32).unwrap(),
            10_u32
        );
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 12_u32);
    }

    #[test]
    fn insert_archetype_change() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let mut u64s = world.new_static_column::<u64>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        u64s.insert_component(&mut world, e, 12_u64).unwrap_none();
        assert_eq!(
            u32s.insert_component(&mut world, e, 15_u32).unwrap(),
            10_u32
        );
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 15_u32);
        assert_eq!(*u64s.get_component(&world, e).unwrap(), 12_u64);
    }

    #[test]
    fn insert_on_dead() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        world.despawn(e);
        u32s.insert_component(&mut world, e, 12_u32).unwrap_none();
    }

    #[test]
    fn basic_remove() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        u32s.remove_component(&mut world, e).unwrap_none();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(u32s.remove_component(&mut world, e).unwrap(), 10_u32);
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn remove_archetype_change() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let mut u64s = world.new_static_column::<u64>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        u64s.insert_component(&mut world, e, 12_u64).unwrap_none();
        assert_eq!(
            u32s.insert_component(&mut world, e, 15_u32).unwrap(),
            10_u32
        );
        u64s.remove_component(&mut world, e);
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 15_u32);
        assert_eq!(u64s.has_component(&world, e), false);
    }

    #[test]
    fn remove_on_dead() {
        let mut world = World::new();
        let mut u32s = world.new_static_column::<u32>();
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        world.despawn(e);
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn non_static() {
        let a = 10;
        let b = &a;
        let mut world = World::new();
        let mut u32borrows = world.new_static_column::<&u32>();
        let e1 = world.spawn().insert(&mut u32borrows, b).id();
        for (entity, data) in &mut ColumnLocks::new((WithEntities, &u32borrows), &world) {
            assert_eq!(entity, e1);
            assert_eq!(*data, b);
            return;
        }
        unreachable!()
    }

    #[test]
    fn drop_called() {
        #[derive(Component)]
        struct Foo<'a>(&'a mut u32);
        impl Drop for Foo<'_> {
            fn drop(&mut self) {
                *self.0 = 12;
            }
        }

        let mut a = 10;
        let b = Foo(&mut a);
        let mut world = World::new();
        let mut u32borrows = world.new_static_column::<Foo<'_>>();
        world.spawn().insert(&mut u32borrows, b);
        drop(u32borrows);
        assert_eq!(a, 12);
    }
}

#[cfg(test)]
mod mismatched_world_id_tests {
    use crate::*;

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn remove_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_static_column::<u32>();
        other_u32s.remove_component(&mut world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn insert_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_static_column::<u32>();
        other_u32s.insert_component(&mut world, e, 10);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn has_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_static_column::<u32>();
        other_u32s.has_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component_mut() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_static_column::<u32>();
        other_u32s.get_component_mut(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_static_column::<u32>();
        other_u32s.get_component(&world, e);
    }
}
