use not_ghost_cell::SlowGhostToken;

use crate::{
    world::{Archetype, Columns, ColumnsApi},
    EcsTypeId, Entity, Joinable, World, WorldId,
};

pub struct RawTable<T>(Vec<Vec<T>>);
pub struct Table<T>(SlowGhostToken<RawTable<T>>, EcsTypeId, WorldId);
impl<T> Table<T> {
    pub fn new<'a>(world: &mut World<'a>) -> Self
    where
        T: 'a,
    {
        let (token, id) = world.new_handle_raw(RawTable::<T>(vec![]));
        Table::<T>(token, id, world.id())
    }
}

impl<T> ColumnsApi for Table<T> {
    type Insert<'a> = T
    where
        Self: 'a;

    type Remove = T;
    type Get = T;

    fn ecs_type_id(&self) -> EcsTypeId {
        self.1
    }
    fn world_id(&self) -> WorldId {
        self.2
    }

    fn get_component_raw<'a>(&'a self, world: &'a World, entity: Entity) -> Option<&'a T> {
        let archetype_id = world.entity_meta(entity)?.archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(self.1)?;
        let cell = world.get_cell(self.1);
        Some(&cell.deref(&self.0).0[column_idx][entity_idx])
    }

    fn get_component_raw_mut<'a>(
        &'a mut self,
        world: &'a World,
        entity: Entity,
    ) -> Option<&'a mut T> {
        let archetype_id = world.entity_meta(entity)?.archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(self.1)?;
        let cell = world.get_cell(self.1);
        Some(&mut cell.deref_mut(&mut self.0).0[column_idx][entity_idx])
    }

    fn insert_component_raw<'a, 'b>(
        &'a mut self,
        world: &'a World,
        entity: Entity,
        data: Self::Insert<'b>,
    ) where
        Self: 'b,
    {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let column_idx = archetype.column_index(self.1).unwrap();
        let cell = world.get_cell(self.1);
        cell.deref_mut(&mut self.0).0[column_idx].push(data);
    }

    fn remove_component_raw<'a>(&'a mut self, world: &'a World, entity: Entity) -> T {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(self.1).unwrap();
        let cell = world.get_cell(self.1);
        cell.deref_mut(&mut self.0).0[column_idx].swap_remove(entity_idx)
    }

    fn insert_overwrite_raw<'a>(overwrite: &mut T, data: T) -> T
    where
        Self: 'a,
    {
        std::mem::replace(&mut *overwrite, data)
    }
}

impl<T> Columns for RawTable<T> {
    fn push_empty_column(&mut self) -> usize {
        self.0.push(vec![]);
        self.0.len() - 1
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

impl<'a, T> Joinable for &'a Table<T> {
    type Ids = EcsTypeId;

    type IterState<'world> = (EcsTypeId, &'world RawTable<T>)
    where
        Self: 'world;

    type ArchetypeState<'world> = std::slice::Iter<'world, T>
    where
        Self: 'world;

    type Item<'world> = &'world T
    where
        Self: 'world;

    fn assert_world_id(&self, world_id: WorldId) {
        crate::assert_world_id(world_id, self.2, std::any::type_name::<Table<T>>())
    }

    fn make_ids(&self, _: &World) -> Self::Ids {
        self.1
    }

    fn make_iter_state<'world>(self, world: &'world World) -> (EcsTypeId, &'world RawTable<T>)
    where
        Self: 'world,
    {
        let id = self.1;
        (id, world.deref_token(&self.0, id))
    }

    fn archetype_matches(id: &EcsTypeId, archetype: &Archetype) -> bool {
        archetype.contains_id(*id)
    }

    fn make_archetype_state<'world>(
        (id, state): &mut (EcsTypeId, &'world RawTable<T>),
        archetype: &'world Archetype,
    ) -> std::slice::Iter<'world, T>
    where
        Self: 'world,
    {
        let col = archetype.column_indices[id];
        state.0[col].iter()
    }

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next()
    }
}

impl<'a, T> Joinable for &'a mut Table<T> {
    type Ids = EcsTypeId;

    type IterState<'world> = (EcsTypeId, usize, &'world mut [Vec<T>])
    where
        Self: 'world;

    type ArchetypeState<'world> = std::slice::IterMut<'world, T>
    where
        Self: 'world;

    type Item<'world> = &'world mut T
    where
        Self: 'world;

    fn assert_world_id(&self, world_id: WorldId) {
        crate::assert_world_id(world_id, self.2, std::any::type_name::<Table<T>>());
    }

    fn make_ids(&self, _: &World) -> Self::Ids {
        self.1
    }

    fn make_iter_state<'world>(
        self,
        world: &'world World,
    ) -> (EcsTypeId, usize, &'world mut [Vec<T>])
    where
        Self: 'world,
    {
        let id = self.1;
        (
            id,
            0,
            world.deref_mut_token(&mut self.0, id).0.as_mut_slice(),
        )
    }

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        archetype.contains_id(*ids)
    }

    fn make_archetype_state<'world>(
        (ecs_type_id, num_chopped_off, lock_borrow): &mut (EcsTypeId, usize, &'world mut [Vec<T>]),
        archetype: &'world Archetype,
    ) -> Self::ArchetypeState<'world>
    where
        Self: 'world,
    {
        let col = archetype.column_indices[ecs_type_id];
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of.last_mut().unwrap().iter_mut()
    }

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next()
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
        let u32s = Table::<u32>::new(&mut world);
        let e = world.spawn().id();
        assert_eq!(u32s.has_component(&world, e), false);
    }

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 10_u32);
    }

    #[test]
    fn insert_overwrite() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
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
        let mut u32s = Table::<u32>::new(&mut world);
        let mut u64s = Table::<u64>::new(&mut world);
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
    #[should_panic = "Unexpected dead entity"]
    fn insert_on_dead() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        world.despawn(e);
        u32s.insert_component(&mut world, e, 12_u32).unwrap_none();
    }

    #[test]
    fn basic_remove() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e = world.spawn().id();
        u32s.remove_component(&mut world, e).unwrap_none();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(u32s.remove_component(&mut world, e).unwrap(), 10_u32);
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn remove_archetype_change() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let mut u64s = Table::<u64>::new(&mut world);
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
    #[should_panic = "Unexpected dead entity"]
    fn remove_on_dead() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
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
        let mut u32borrows = Table::<&u32>::new(&mut world);
        let e1 = world.spawn().insert(&mut u32borrows, b).id();
        for (entity, data) in world.join((WithEntities, &u32borrows)) {
            assert_eq!(entity, e1);
            assert_eq!(*data, b);
            return;
        }
        unreachable!()
    }

    #[test]
    fn drop_called() {
        struct Foo<'a>(&'a mut u32);
        impl Drop for Foo<'_> {
            fn drop(&mut self) {
                *self.0 = 12;
            }
        }

        let mut a = 10;
        let b = Foo(&mut a);
        let mut world = World::new();
        let mut u32borrows = Table::<Foo<'_>>::new(&mut world);
        world.spawn().insert(&mut u32borrows, b);
        drop(u32borrows);
        assert_eq!(a, 12);
    }

    #[test]
    fn get_component() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e = world.spawn().insert(&mut u32s, 10_u32).id();
        let data = u32s.get_component_mut(&world, e).unwrap();
        assert_eq!(*data, 10_u32);
    }

    #[test]
    fn for_loop() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e1 = world.spawn().insert(&mut u32s, 10).id();
        for data in world.join((WithEntities, &u32s)) {
            assert_eq!(data, (e1, &10));
            return;
        }
        unreachable!()
    }

    #[test]
    fn simple_query() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let mut u64s = Table::<u64>::new(&mut world);
        let mut u128s = Table::<u128>::new(&mut world);
        world
            .spawn()
            .insert(&mut u32s, 10_u32)
            .insert(&mut u64s, 12_u64)
            .id();
        world
            .spawn()
            .insert(&mut u64s, 13_u64)
            .insert(&mut u128s, 9_u128)
            .id();
        let returned = world.join(&u64s).collect::<Vec<_>>();
        assert_eq!(returned, [&12, &13]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let mut u64s = Table::<u64>::new(&mut world);
        let mut u128s = Table::<u128>::new(&mut world);
        let e1 = world
            .spawn()
            .insert(&mut u32s, 10_u32)
            .insert(&mut u64s, 12_u64)
            .id();
        world
            .spawn()
            .insert(&mut u64s, 13_u64)
            .insert(&mut u128s, 9_u128)
            .id();
        let returned = world.join((WithEntities, &u32s, &u64s)).collect::<Vec<_>>();
        assert_eq!(returned, [(e1, &10, &12)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let mut u64s = Table::<u64>::new(&mut world);
        let mut u128s = Table::<u128>::new(&mut world);

        let e1 = world
            .spawn()
            .insert(&mut u32s, 10_u32)
            .insert(&mut u64s, 12_u64)
            .id();
        let e2 = world
            .spawn()
            .insert(&mut u64s, 13_u64)
            .insert(&mut u128s, 9_u128)
            .id();

        let returned = world
            .join((WithEntities, Maybe(&u32s), &u64s, Maybe(&u128s)))
            .collect::<Vec<_>>();
        assert_eq!(
            returned,
            [
                (e1, Some(&10_u32), &12_u64, None),
                (e2, None, &13_u64, Some(&9_u128))
            ],
        )
    }

    #[test]
    fn query_with_despawned() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let e1 = world.spawn().insert(&mut u32s, 10_u32).id();
        world.despawn(e1);

        let mut iter = world.join(&u32s);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        let mut u32s = Table::<u32>::new(&mut world);
        let u64s = Table::<u64>::new(&mut world);
        let e1 = world.spawn().insert(&mut u32s, 10_u32).id();
        let e2 = world.spawn().insert(&mut u32s, 12_u32).id();
        let returned = world
            .join((WithEntities, Maybe(&u64s), &u32s))
            .collect::<Vec<_>>();
        assert_eq!(returned, [(e1, None, &10_u32), (e2, None, &12_u32)]);
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
        let mut other_u32s = Table::<u32>::new(&mut other_world);
        other_u32s.remove_component(&mut world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn insert_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = Table::<u32>::new(&mut other_world);
        other_u32s.insert_component(&mut world, e, 10);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn has_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        other_u32s.has_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component_mut() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = Table::<u32>::new(&mut other_world);
        other_u32s.get_component_mut(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        other_u32s.get_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn ref_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        world.join(&other_u32s);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn mut_join() {
        let world = World::new();
        let mut other_world = World::new();
        let mut other_u32s = Table::<u32>::new(&mut other_world);
        world.join(&mut other_u32s);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn maybe_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        world.join(Maybe(&other_u32s));
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn unsatisfied_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        world.join(Unsatisfied(&other_u32s));
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn multi_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = Table::<u32>::new(&mut other_world);
        world.join((WithEntities, &other_u32s));
    }
}
