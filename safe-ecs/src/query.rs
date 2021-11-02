use crate::{
    sealed,
    system::Access,
    world::{Archetype, Storage},
    Component, Entity, World,
};
use std::{any::TypeId, cell, marker::PhantomData};

pub trait QueryParam: sealed::Sealed + 'static {
    type Lock<'a>;
    type LockBorrow<'a>;
    type Item<'a>;
    type ItemIter<'a>;
    fn lock_from_world(world: &World) -> Self::Lock<'_>;
    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a>;
    fn archetype_matches(archetype: &Archetype) -> bool;
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a>;
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>>;
    fn get_access() -> Result<Access, ()>;
}

impl sealed::Sealed for Entity {}
impl QueryParam for Entity {
    type Lock<'a> = ();
    type LockBorrow<'a> = ();
    type Item<'a> = Entity;
    type ItemIter<'a> = std::slice::Iter<'a, Entity>;

    fn lock_from_world(_: &World) -> Self::Lock<'_> {}
    fn lock_borrows_from_locks<'a, 'b>(_: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {}
    fn archetype_matches(_: &Archetype) -> bool {
        true
    }
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        _: &mut Self::LockBorrow<'a>,
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

impl<T: Component> sealed::Sealed for &'static T {}
impl<T: Component> QueryParam for &'static T {
    type Lock<'a> = cell::Ref<'a, Vec<Box<dyn Storage>>>;
    type LockBorrow<'a> = &'a [Box<dyn Storage>];
    type Item<'a> = &'a T;
    type ItemIter<'a> = std::slice::Iter<'a, T>;

    fn lock_from_world(world: &World) -> Self::Lock<'_> {
        // FIXME, two panics
        (world.columns[&TypeId::of::<T>()]).borrow()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        lock.as_slice()
    }

    fn archetype_matches(archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(&TypeId::of::<T>())
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a> {
        let col = archetype.column_indices[&TypeId::of::<T>()];
        lock_borrow[col].as_vec::<T>().unwrap().iter()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
    }

    fn get_access() -> Result<Access, ()> {
        Access::new().insert_read(TypeId::of::<T>())
    }
}

impl<T: Component> sealed::Sealed for &'static mut T {}
impl<T: Component> QueryParam for &'static mut T {
    type Lock<'a> = cell::RefMut<'a, Vec<Box<dyn Storage>>>;
    type LockBorrow<'a> = (usize, &'a mut [Box<dyn Storage>]);
    type Item<'a> = &'a mut T;
    type ItemIter<'a> = std::slice::IterMut<'a, T>;

    fn lock_from_world(world: &World) -> Self::Lock<'_> {
        // FIXME, two panics
        (world.columns[&TypeId::of::<T>()]).borrow_mut()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        (0, lock.as_mut_slice())
    }

    fn archetype_matches(archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(&TypeId::of::<T>())
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        (num_chopped_off, lock_borrow): &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a> {
        let col = archetype.column_indices[&TypeId::of::<T>()];
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of
            .last_mut()
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
        impl<$($T: QueryParam),+> sealed::Sealed for ($($T,)+) {}
        impl<$($T: QueryParam),+> QueryParam for ($($T,)+) {
            type Lock<'a> = ($($T::Lock<'a>,)+);
            type LockBorrow<'a> = ($($T::LockBorrow<'a>,)+);
            type Item<'a> = ($($T::Item<'a>,)+);
            type ItemIter<'a> = ($($T::ItemIter<'a>,)+);

            fn lock_from_world(world: &World) -> Self::Lock<'_> {
                ($($T::lock_from_world(world),)+)
            }

            #[allow(non_snake_case)]
            fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
                let ($($T,)+) = lock;
                ($($T::lock_borrows_from_locks($T),)+)
            }

            fn archetype_matches(archetype: &Archetype) -> bool {
                $($T::archetype_matches(archetype))&&+
            }

            #[allow(non_snake_case)]
            fn item_iter_from_archetype<'a>(archetype: &'a Archetype, lock_borrow: &mut Self::LockBorrow<'a>) -> Self::ItemIter<'a> {
                let ($($T,)+) = lock_borrow;
                ($($T::item_iter_from_archetype(archetype, $T),)+)
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

impl<Q: QueryParam> sealed::Sealed for Maybe<Q> {}
pub struct Maybe<Q: QueryParam>(PhantomData<Q>);
pub enum MaybeIter<'a, Q: QueryParam> {
    Some(Q::ItemIter<'a>),
    None(usize),
}
impl<Q: QueryParam> QueryParam for Maybe<Q> {
    type Lock<'a> = Q::Lock<'a>;
    type LockBorrow<'a> = Q::LockBorrow<'a>;
    type Item<'a> = Option<Q::Item<'a>>;
    type ItemIter<'a> = MaybeIter<'a, Q>;

    fn lock_from_world(world: &World) -> Self::Lock<'_> {
        Q::lock_from_world(world)
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        Q::lock_borrows_from_locks(lock)
    }

    fn archetype_matches(_: &Archetype) -> bool {
        true
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a> {
        match Q::archetype_matches(archetype) {
            true => MaybeIter::Some(Q::item_iter_from_archetype(archetype, lock_borrow)),
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

pub struct Query<'a, Q: QueryParam + 'static>(pub(crate) &'a World, pub(crate) Q::Lock<'a>);
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
    borrows: Q::LockBorrow<'a>,
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
                .filter(|archetype| Q::archetype_matches(archetype))
        }

        Self {
            archetype_iter: defining_use::<Q>(borrows.0),
            borrows: Q::lock_borrows_from_locks(&mut borrows.1),
            item_iters: None,
        }
    }
}

impl<'a, 'b: 'a, Q: QueryParam> Iterator for QueryIter<'a, 'b, Q> {
    type Item = Q::Item<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let None = &self.item_iters {
                let archetype = self.archetype_iter.next()?;
                self.item_iters = Some(Q::item_iter_from_archetype(archetype, &mut self.borrows));
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

        let mut q = world.query::<&u64>();
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

        let mut q = world.query::<(Entity, &u32, &u64)>();
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

        let mut q = world.query::<(Entity, Maybe<&u32>, &u64, Maybe<&u128>)>();
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

        let mut q = world.query::<&u32>();
        q.iter_mut().for_each(|_| unreachable!());
    }
}
