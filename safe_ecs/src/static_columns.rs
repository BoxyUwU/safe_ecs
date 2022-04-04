use crate::{
    world::{Archetype, Columns, ColumnsApi},
    EcsTypeId, Entity, IterableColumns, World,
};

pub struct Table<T>(pub(crate) Vec<Vec<T>>);
impl<T> Table<T> {
    pub fn new() -> Self {
        Self::default()
    }
}
impl<T> Default for Table<T> {
    fn default() -> Self {
        Table(vec![])
    }
}

impl<T> ColumnsApi for Table<T> {
    type Insert<'a> = T
    where
        Self: 'a;

    type Remove = T;
    type Get = T;

    fn get_component<'a>(&'a self, world: &World, id: EcsTypeId, entity: Entity) -> Option<&'a T> {
        let archetype_id = world.entity_meta(entity)?.archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;
        Some(&self.0[column_idx][entity_idx])
    }

    fn get_component_mut<'a>(
        &'a mut self,
        world: &World,
        id: EcsTypeId,
        entity: Entity,
    ) -> Option<&'a mut T> {
        let archetype_id = world.entity_meta(entity)?.archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;
        Some(&mut self.0[column_idx][entity_idx])
    }

    fn insert_component<'a, 'b>(
        &'a mut self,
        world: &mut World,
        id: EcsTypeId,
        entity: Entity,
        data: Self::Insert<'b>,
    ) where
        Self: 'b,
    {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let column_idx = archetype.column_index(id).unwrap();
        self.0[column_idx].push(data);
    }

    fn remove_component<'a>(&'a mut self, world: &mut World, id: EcsTypeId, entity: Entity) -> T {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id).unwrap();
        self.0[column_idx].swap_remove(entity_idx)
    }

    fn insert_overwrite<'a>(overwrite: &mut T, data: T) -> T
    where
        Self: 'a,
    {
        std::mem::replace(&mut *overwrite, data)
    }
}

impl<T> Columns for Table<T> {
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

impl<'a, T> IterableColumns for &'a Table<T> {
    type Item = &'a T;
    type IterState = (EcsTypeId, &'a [Vec<T>]);
    type ArchetypeState = std::slice::Iter<'a, T>;

    fn make_iter_state(id: EcsTypeId, locks: &'a Table<T>) -> (EcsTypeId, &'a [Vec<T>]) {
        (id, &locks.0[..])
    }

    fn make_archetype_state<'lock>(
        (id, state): &mut Self::IterState,
        archetype: &'lock Archetype,
    ) -> Self::ArchetypeState
    where
        Self: 'lock,
    {
        let col = archetype.column_indices[id];
        state[col].iter()
    }

    fn item_of_entity(iter: &mut Self::ArchetypeState) -> Option<Self::Item> {
        iter.next()
    }
}

impl<'a, T> IterableColumns for &'a mut Table<T> {
    type Item = &'a mut T;
    type IterState = (EcsTypeId, usize, &'a mut [Vec<T>]);
    type ArchetypeState = std::slice::IterMut<'a, T>;

    fn make_iter_state(id: EcsTypeId, locks: Self) -> Self::IterState {
        (id, 0, &mut locks.0[..])
    }

    fn make_archetype_state<'lock>(
        (ecs_type_id, num_chopped_off, lock_borrow): &mut Self::IterState,
        archetype: &'lock Archetype,
    ) -> Self::ArchetypeState
    where
        Self: 'lock,
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

    fn item_of_entity(iter: &mut Self::ArchetypeState) -> Option<Self::Item> {
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
        let u32s = world.new_handle(Table::<u32>::new());
        let e = world.spawn().id();
        assert_eq!(u32s.has_component(&world, e), false);
    }

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(*u32s.get_component(&world, e).unwrap(), 10_u32);
    }

    #[test]
    fn insert_overwrite() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
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
        let mut u32s = world.new_handle(Table::<u32>::new());
        let mut u64s = world.new_handle(Table::<u64>::new());
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
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        world.despawn(e);
        u32s.insert_component(&mut world, e, 12_u32).unwrap_none();
    }

    #[test]
    fn basic_remove() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e = world.spawn().id();
        u32s.remove_component(&mut world, e).unwrap_none();
        u32s.insert_component(&mut world, e, 10_u32).unwrap_none();
        assert_eq!(u32s.remove_component(&mut world, e).unwrap(), 10_u32);
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn remove_archetype_change() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let mut u64s = world.new_handle(Table::<u64>::new());
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
        let mut u32s = world.new_handle(Table::<u32>::new());
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
        let mut u32borrows = world.new_handle(Table::<&u32>::new());
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
        struct Foo<'a>(&'a mut u32);
        impl Drop for Foo<'_> {
            fn drop(&mut self) {
                *self.0 = 12;
            }
        }

        let mut a = 10;
        let b = Foo(&mut a);
        let mut world = World::new();
        let mut u32borrows = world.new_handle(Table::<Foo<'_>>::new());
        world.spawn().insert(&mut u32borrows, b);
        drop(u32borrows);
        assert_eq!(a, 12);
    }

    #[test]
    fn get_component() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e = world.spawn().insert(&mut u32s, 10_u32).id();
        let data = u32s.get_component_mut(&world, e).unwrap();
        assert_eq!(*data, 10_u32);
    }

    #[test]
    fn for_loop() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e1 = world.spawn().insert(&mut u32s, 10).id();
        for data in ColumnLocks::new((WithEntities, &u32s), &world).into_iter() {
            assert_eq!(data, (e1, &10));
            return;
        }
        unreachable!()
    }

    #[test]
    fn simple_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let mut u64s = world.new_handle(Table::<u64>::new());
        let mut u128s = world.new_handle(Table::<u128>::new());
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
        let mut locks = ColumnLocks::new(&u64s, &world);
        let returned = locks.into_iter().collect::<Vec<_>>();
        assert_eq!(returned, [&12, &13]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let mut u64s = world.new_handle(Table::<u64>::new());
        let mut u128s = world.new_handle(Table::<u128>::new());
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
        let mut locks = ColumnLocks::new((WithEntities, &u32s, &u64s), &world);
        let returned = locks.into_iter().collect::<Vec<_>>();
        assert_eq!(returned, [(e1, &10, &12)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let mut u64s = world.new_handle(Table::<u64>::new());
        let mut u128s = world.new_handle(Table::<u128>::new());

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

        let mut locks =
            ColumnLocks::new((WithEntities, Maybe(&u32s), &u64s, Maybe(&u128s)), &world);
        let returned = locks.into_iter().collect::<Vec<_>>();
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
        let mut u32s = world.new_handle(Table::<u32>::new());
        let e1 = world.spawn().insert(&mut u32s, 10_u32).id();
        world.despawn(e1);

        let mut locks = ColumnLocks::new(&u32s, &world);
        let mut iter = locks.into_iter();
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(Table::<u32>::new());
        let u64s = world.new_handle(Table::<u64>::new());
        let e1 = world.spawn().insert(&mut u32s, 10_u32).id();
        let e2 = world.spawn().insert(&mut u32s, 12_u32).id();
        let mut locks = ColumnLocks::new((WithEntities, Maybe(&u64s), &u32s), &world);
        let returned = locks.into_iter().collect::<Vec<_>>();
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
        let mut other_u32s = other_world.new_handle(Table::<u32>::new());
        other_u32s.remove_component(&mut world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn insert_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(Table::<u32>::new());
        other_u32s.insert_component(&mut world, e, 10);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn has_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        other_u32s.has_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component_mut() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(Table::<u32>::new());
        other_u32s.get_component_mut(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        other_u32s.get_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn ref_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        ColumnLocks::new(&other_u32s, &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn mut_join() {
        let world = World::new();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(Table::<u32>::new());
        ColumnLocks::new(&mut other_u32s, &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn maybe_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        ColumnLocks::new(Maybe(&other_u32s), &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn unsatisfied_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        ColumnLocks::new(Unsatisfied(&other_u32s), &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn multi_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(Table::<u32>::new());
        ColumnLocks::new((WithEntities, &other_u32s), &world);
    }
}
