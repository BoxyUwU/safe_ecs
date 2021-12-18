use crate::{
    errors::WorldBorrowError,
    system::Access,
    world::{Archetype, EcsTypeId, Storage},
    Component, Entity, World,
};
use std::{
    any::{type_name, TypeId},
    cell,
    collections::HashMap,
    marker::PhantomData,
};

pub trait QueryParam: 'static {
    type Lock<'a>
    where
        Self: 'a;
    type LockBorrow<'a>
    where
        Self: 'a;
    type Item<'a>
    where
        Self: 'a;
    type ItemIter<'a>
    where
        Self: 'a;
    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError>;
    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a>;
    fn archetype_matches(archetype: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool;
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        _: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a>;
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>>;
    fn get_access() -> Result<Access, ()>;
}

impl QueryParam for Entity {
    type Lock<'a> = ();
    type LockBorrow<'a> = ();
    type Item<'a> = Entity;
    type ItemIter<'a> = std::slice::Iter<'a, Entity>;

    fn lock_from_world(_: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        Ok(Some(()))
    }
    fn lock_borrows_from_locks<'a, 'b>(_: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {}
    fn archetype_matches(_: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool {
        true
    }
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        _: &mut Self::LockBorrow<'a>,
        _: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        archetype.entities.iter()
    }
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next().copied()
    }
    fn get_access() -> Result<Access, ()> {
        Ok(Access::new())
    }
}

impl<T: Component> QueryParam for &'static T {
    type Lock<'a> = cell::Ref<'a, Vec<Box<dyn Storage>>>;
    type LockBorrow<'a> = &'a [Box<dyn Storage>];
    type Item<'a> = &'a T;
    type ItemIter<'a> = std::slice::Iter<'a, T>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        let ecs_type_id = match world.ecs_type_ids.get(&TypeId::of::<T>()) {
            None => return Ok(None),
            Some(ecs_type_id) => ecs_type_id,
        };

        world
            .columns
            .get(ecs_type_id)
            .map(|cell| {
                cell.try_borrow()
                    .map_err(|_| WorldBorrowError(type_name::<T>()))
            })
            .transpose()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        lock.as_slice()
    }

    fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
        let ecs_type_id = match ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return false,
        };
        archetype.column_indices.contains_key(ecs_type_id)
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        let ecs_type_id = ecs_type_ids.get(&TypeId::of::<T>()).unwrap();
        let col = archetype.column_indices[ecs_type_id];
        lock_borrow[col]
            .as_typed_storage()
            .unwrap()
            .as_vec::<T>()
            .unwrap()
            .iter()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
    }

    fn get_access() -> Result<Access, ()> {
        Access::new().insert_read(TypeId::of::<T>())
    }
}

impl<T: Component> QueryParam for &'static mut T {
    type Lock<'a> = cell::RefMut<'a, Vec<Box<dyn Storage>>>;
    type LockBorrow<'a> = (usize, &'a mut [Box<dyn Storage>]);
    type Item<'a> = &'a mut T;
    type ItemIter<'a> = std::slice::IterMut<'a, T>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        let ecs_type_id = match world.ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return Ok(None),
        };

        world
            .columns
            .get(ecs_type_id)
            .map(|cell| {
                cell.try_borrow_mut()
                    .map_err(|_| WorldBorrowError(type_name::<T>()))
            })
            .transpose()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        (0, lock.as_mut_slice())
    }

    fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
        let ecs_type_id = match ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return false,
        };
        archetype.column_indices.contains_key(ecs_type_id)
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        (num_chopped_off, lock_borrow): &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        let ecs_type_id = ecs_type_ids.get(&TypeId::of::<T>()).unwrap();

        let col = archetype.column_indices[ecs_type_id];
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of
            .last_mut()
            .unwrap()
            .as_typed_storage_mut()
            .unwrap()
            .as_vec_mut::<T>()
            .unwrap()
            .iter_mut()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
    }

    fn get_access() -> Result<Access, ()> {
        Access::new().insert_write(TypeId::of::<T>())
    }
}

macro_rules! query_param_tuple_impl {
    ($($T:ident)+) => {
        impl<$($T: QueryParam),+> QueryParam for ($($T,)+) {
            type Lock<'a> = ($($T::Lock<'a>,)+);
            type LockBorrow<'a> = ($($T::LockBorrow<'a>,)+);
            type Item<'a> = ($($T::Item<'a>,)+);
            type ItemIter<'a> = ($($T::ItemIter<'a>,)+);

            fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
                Ok(Some(
                    ($(
                        match $T::lock_from_world(world)? {
                            None => return Ok(None),
                            Some(lock) => lock,
                        },
                    )+)
                ))
            }

            #[allow(non_snake_case)]
            fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
                let ($($T,)+) = lock;
                ($($T::lock_borrows_from_locks($T),)+)
            }

            fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
                $($T::archetype_matches(archetype, ecs_type_ids))&&+
            }

            #[allow(non_snake_case)]
            fn item_iter_from_archetype<'a>(
                archetype: &'a Archetype,
                lock_borrow: &mut Self::LockBorrow<'a>,
                ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
            ) -> Self::ItemIter<'a> {
                let ($($T,)+) = lock_borrow;
                ($($T::item_iter_from_archetype(archetype, $T, ecs_type_ids),)+)
            }

            #[allow(non_snake_case)]
            fn advance_iter<'a>(iters: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
                let ($($T,)+) = iters;
                Some(($($T::advance_iter($T)?,)+))
            }

            fn get_access() -> Result<Access, ()> {
                Access::from_array([$($T::get_access()),+])
            }
        }
    };
}

query_param_tuple_impl!(A B C D E F G H);
query_param_tuple_impl!(A B C D E F G);
query_param_tuple_impl!(A B C D E F);
query_param_tuple_impl!(A B C D E);
query_param_tuple_impl!(A B C D);
query_param_tuple_impl!(A B C);
query_param_tuple_impl!(A B);
query_param_tuple_impl!(A);

pub struct Maybe<Q: QueryParam>(PhantomData<Q>);
pub enum MaybeIter<'a, Q: QueryParam> {
    Some(Q::ItemIter<'a>),
    None(usize),
}
impl<Q: QueryParam> QueryParam for Maybe<Q> {
    type Lock<'a> = Option<Q::Lock<'a>>;
    type LockBorrow<'a> = Option<Q::LockBorrow<'a>>;
    type Item<'a> = Option<Q::Item<'a>>;
    type ItemIter<'a> = MaybeIter<'a, Q>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        Ok(Some(Q::lock_from_world(world)?))
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        lock.as_mut()
            .map(|q_lock| Q::lock_borrows_from_locks(q_lock))
    }

    fn archetype_matches(_: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool {
        true
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        match Q::archetype_matches(archetype, ecs_type_ids) {
            true => MaybeIter::Some(Q::item_iter_from_archetype(
                archetype,
                lock_borrow.as_mut().unwrap(),
                ecs_type_ids,
            )),
            false => MaybeIter::None(archetype.entities.len()),
        }
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        match iter {
            MaybeIter::Some(iter) => Q::advance_iter(iter).map(|item| Some(item)),
            MaybeIter::None(0) => None,
            MaybeIter::None(remaining) => {
                *remaining -= 1;
                Some(None)
            }
        }
    }

    fn get_access() -> Result<Access, ()> {
        Q::get_access()
    }
}

pub struct Query<'a, Q: QueryParam + 'static>(pub(crate) &'a World, pub(crate) Option<Q::Lock<'a>>);
impl<'b, Q: QueryParam> Query<'b, Q> {
    pub fn iter_mut(&mut self) -> QueryIter<'_, 'b, Q> {
        QueryIter::new(self)
    }
}
impl<'a, 'b: 'a, Q: QueryParam> IntoIterator for &'a mut Query<'b, Q> {
    type Item = Q::Item<'a>;
    type IntoIter = QueryIter<'a, 'b, Q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub struct QueryIter<'a, 'b: 'a, Q: QueryParam> {
    ecs_type_ids: &'a HashMap<TypeId, EcsTypeId>,
    /// `None` if we couldnt acquire the locks because
    /// one of the columns had not been created yet
    borrows: Option<Q::LockBorrow<'a>>,
    archetype_iter: ArchetypeIter<'b, Q>,
    item_iters: Option<Q::ItemIter<'a>>,
}

type ArchetypeIter<'b, Q> = impl Iterator<Item = &'b Archetype> + 'b;
impl<'a, 'b: 'a, Q: QueryParam> QueryIter<'a, 'b, Q> {
    fn new(borrows: &'a mut Query<'b, Q>) -> Self {
        fn defining_use<'b, Q: QueryParam>(world: &'b World) -> ArchetypeIter<'b, Q> {
            world
                .archetypes
                .iter()
                .filter(|archetype| Q::archetype_matches(archetype, &world.ecs_type_ids))
        }

        Self {
            ecs_type_ids: &borrows.0.ecs_type_ids,
            archetype_iter: defining_use::<Q>(borrows.0),
            borrows: borrows
                .1
                .as_mut()
                .map(|locks| Q::lock_borrows_from_locks(locks)),
            item_iters: None,
        }
    }
}

impl<'a, 'b: 'a, Q: QueryParam> Iterator for QueryIter<'a, 'b, Q> {
    type Item = Q::Item<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let borrows = self.borrows.as_mut()?;
        loop {
            if let None = &self.item_iters {
                let archetype = self.archetype_iter.next()?;
                self.item_iters = Some(Q::item_iter_from_archetype(
                    archetype,
                    borrows,
                    self.ecs_type_ids,
                ));
            }

            match Q::advance_iter(self.item_iters.as_mut().unwrap()) {
                Some(item) => return Some(item),
                None => self.item_iters = None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::*;

    #[test]
    fn simple_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world.query::<&u64>().unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[&12, &13]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world.query::<(Entity, &u32, &u64)>().unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[(e1, &10, &12)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world
            .query::<(Entity, Maybe<&u32>, &u64, Maybe<&u128>)>()
            .unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(
            returned.as_slice(),
            &[
                (e1, Some(&10_u32), &12_u64, None),
                (e2, None, &13_u64, Some(&9_u128))
            ],
        )
    }

    #[test]
    fn query_with_despawned() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).id();
        world.despawn(e1);

        let mut q = world.query::<&u32>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn conflicting_queries() {
        let mut world = World::new();
        let _e1 = world.spawn().insert(10_u32).insert(10_u64).id();

        let _q1 = world.query::<&u32>().unwrap();
        assert!(matches!(world.query::<&mut u32>(), Err(_)));

        let _q2 = world.query::<&mut u64>().unwrap();
        assert!(matches!(world.query::<(&u32, &mut u64)>(), Err(_)));

        let _q3 = world.query::<&u32>().unwrap();
    }

    #[test]
    fn maybe_on_uncreated_column() {
        let mut world = World::new();
        let _e1 = world.spawn().id();
        let mut q = world.query::<Maybe<&u32>>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some(None));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).id();
        let e2 = world.spawn().insert(12_u32).id();
        let mut q = world.query::<(Entity, Maybe<&u64>, &u32)>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some((e1, None, &10_u32)));
        assert_eq!(iter.next(), Some((e2, None, &12_u32)));
        assert_eq!(iter.next(), None);
    }
}
