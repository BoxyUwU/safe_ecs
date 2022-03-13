use crate::world::Archetype;
use crate::StaticColumns;
use crate::{Component, EcsTypeId, Entity, World};

/// This trait is also implemented for tuples up to length 8 where all elements implement this trait
pub trait Joinable {
    type Ids;
    type State;
    type Item;
    type ItemIter<'arch>;
    fn make_ids(&self) -> Self::Ids;
    fn make_state(self) -> Self::State;
    fn iter_from_archetype<'arch>(
        state: &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch>;
    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool;
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item>;
}

impl<'a, T: Component> Joinable for &'a StaticColumns<T> {
    type Ids = EcsTypeId;
    type State = (EcsTypeId, &'a [Vec<T>]);
    type Item = &'a T;
    type ItemIter<'arch> = std::slice::Iter<'a, T>;
    fn make_ids(&self) -> Self::Ids {
        todo!()
        // self.1
    }
    fn make_state(self) -> Self::State {
        todo!()
        // (self.1, &self.0[..])
    }
    fn iter_from_archetype<'arch>(
        (id, state): &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch> {
        let col = archetype.column_indices[id];
        state[col].iter()
    }
    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(ids)
    }
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
        iter.next()
    }
}

impl<'a, T: Component> Joinable for &'a mut StaticColumns<T> {
    type Ids = EcsTypeId;
    type State = (EcsTypeId, usize, &'a mut [Vec<T>]);
    type Item = &'a mut T;
    type ItemIter<'arch> = std::slice::IterMut<'a, T>;
    fn make_ids(&self) -> Self::Ids {
        todo!()
        // self.1
    }
    fn make_state(self) -> Self::State {
        todo!()
        // (self.1, 0, &mut self.0[..])
    }
    fn iter_from_archetype<'arch>(
        (ecs_type_id, num_chopped_off, lock_borrow): &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch> {
        let col = archetype.column_indices[ecs_type_id];
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of.last_mut().unwrap().iter_mut()
    }
    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(ids)
    }
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
        iter.next()
    }
}

pub struct WithEntities;
impl Joinable for WithEntities {
    type Ids = ();
    type State = ();
    type Item = Entity;
    type ItemIter<'arch> = std::slice::Iter<'arch, Entity>;
    fn make_ids(&self) -> Self::Ids {}
    fn make_state(self) -> Self::State {}
    fn iter_from_archetype<'arch>(
        _: &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch> {
        archetype.entities.iter()
    }
    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
        iter.next().copied()
    }
}

pub struct Maybe<J: Joinable>(pub J);
pub enum Either<T, U> {
    T(T),
    U(U),
}
impl<J: Joinable> Joinable for Maybe<J> {
    type Ids = ();
    type State = (J::Ids, J::State);
    type Item = Option<J::Item>;
    type ItemIter<'arch> = Either<J::ItemIter<'arch>, core::ops::Range<usize>>;
    fn make_ids(&self) -> Self::Ids {}
    fn make_state(self) -> Self::State {
        (self.0.make_ids(), self.0.make_state())
    }
    fn iter_from_archetype<'arch>(
        (ids, state): &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch> {
        match J::archetype_matches(ids, archetype) {
            true => Either::T(J::iter_from_archetype(state, archetype)),
            false => Either::U(0..archetype.entities.len()),
        }
    }
    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
        match iter {
            Either::T(t) => J::advance_iter(t).map(Some),
            Either::U(u) => u.next().map(|_| None),
        }
    }
}

pub struct Unsatisfied<J: Joinable>(J);
impl<J: Joinable> Joinable for Unsatisfied<J> {
    type Ids = J::Ids;
    type State = J::State;
    type Item = ();
    type ItemIter<'arch> = core::ops::Range<usize>;
    fn make_ids(&self) -> Self::Ids {
        self.0.make_ids()
    }
    fn make_state(self) -> Self::State {
        self.0.make_state()
    }
    fn iter_from_archetype<'arch>(
        _: &mut Self::State,
        archetype: &'arch Archetype,
    ) -> Self::ItemIter<'arch> {
        0..archetype.entities.len()
    }
    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        J::archetype_matches(ids, archetype) == false
    }
    fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
        iter.next().map(|_| ())
    }
}

macro_rules! tuple_impls_joinable {
    ($($T:ident)*) => {
        #[doc(hidden)]
        #[allow(unused_parens)]
        #[allow(non_snake_case)]
        impl<$($T: Joinable),*> Joinable for ($($T,)*) {
            type Ids = ($($T::Ids,)*);
            type State = ($($T::State,)*);
            type Item = ($($T::Item,)*);
            type ItemIter<'arch> = ($($T::ItemIter<'arch>,)*);
            fn make_ids(&self) -> Self::Ids {
                let ($($T,)*) = self;
                ($($T::make_ids($T),)*)
            }
            fn make_state(self) -> Self::State {
                let ($($T,)*) = self;
                ($($T::make_state($T),)*)
            }
            fn iter_from_archetype<'arch>(state: &mut Self::State, archetype: &'arch Archetype) -> Self::ItemIter<'arch> {
                let ($($T,)*) = state;
                ($($T::iter_from_archetype($T, archetype),)*)
            }
            fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
                let ($($T,)*) = ids;
                true $( && $T::archetype_matches($T, archetype) )*
            }
            fn advance_iter(iter: &mut Self::ItemIter<'_>) -> Option<Self::Item> {
                let ($($T,)*) = iter;
                Some(($($T::advance_iter($T)?,)*))
            }
        }
    };
}

tuple_impls_joinable!(A B C D E F G H);
tuple_impls_joinable!(A B C D E F G);
tuple_impls_joinable!(A B C D E F);
tuple_impls_joinable!(A B C D E);
tuple_impls_joinable!(A B C D);
tuple_impls_joinable!(A B C);
tuple_impls_joinable!(A B);
tuple_impls_joinable!(A);

pub struct ColumnIterator<'a, C: Joinable> {
    archetype_iter: ArchetypeIter<'a, C>,
    state: C::State,
    column_iter: Option<C::ItemIter<'a>>,
}

type ArchetypeIter<'a, C: Joinable> = impl Iterator<Item = &'a Archetype>;
impl<'a, C: Joinable> ColumnIterator<'a, C> {
    pub fn new(borrows: C, world: &'a World) -> Self {
        let ids = C::make_ids(&borrows);
        let state = C::make_state(borrows);

        fn defining_use<'a, C: Joinable>(world: &'a World, ids: C::Ids) -> ArchetypeIter<'a, C> {
            world
                .archetypes
                .iter()
                .filter(move |archetype| C::archetype_matches(&ids, archetype))
        }

        ColumnIterator {
            archetype_iter: defining_use::<C>(world, ids),
            state,
            column_iter: None,
        }
    }
}

impl<'a, C: Joinable> Iterator for ColumnIterator<'a, C> {
    type Item = C::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(iter) = &mut self.column_iter else {
                let archetype = self.archetype_iter.next()?;
                let iter = C::iter_from_archetype(&mut self.state, archetype);
                self.column_iter = Some(iter);
                continue;
            };

            let Some(v) = C::advance_iter(iter) else {
                self.column_iter = None;
                continue;
            };
            return Some(v);
        }
    }
}

#[cfg(testa)]
mod static_tests {
    use crate::*;

    #[test]
    fn simple_query() {
        let mut world = World::new();
        world.spawn().insert(10_u32).insert(12_u64).id();
        world.spawn().insert(13_u64).insert(9_u128).id();
        let q = &*world.borrow::<u64>().unwrap();
        let returned = ColumnIterator::new(q, &world).collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[&12, &13]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).insert(12_u64).id();
        world.spawn().insert(13_u64).insert(9_u128).id();
        let u32s = &*world.borrow::<u32>().unwrap();
        let u64s = &*world.borrow::<u64>().unwrap();
        let returned = ColumnIterator::new((WithEntities, u32s, u64s), &world).collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[(e1, &10, &12)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).insert(12_u64).id();
        let e2 = world.spawn().insert(13_u64).insert(9_u128).id();

        let u32s = &*world.borrow::<u32>().unwrap();
        let u64s = &*world.borrow::<u64>().unwrap();
        let u128s = &*world.borrow::<u128>().unwrap();
        let returned = ColumnIterator::new((WithEntities, Maybe(u32s), u64s, Maybe(u128s)), &world)
            .collect::<Vec<_>>();
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

        let q = &*world.borrow::<u32>().unwrap();
        let mut iter = ColumnIterator::new(q, &world);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn conflicting_queries() {
        let mut world = World::new();
        let _e1 = world.spawn().insert(10_u32).insert(10_u64).id();

        let _q1 = world.borrow::<u32>().unwrap();
        assert!(matches!(world.borrow_mut::<u32>(), Err(_)));

        let _q2 = world.borrow_mut::<u64>().unwrap();
        world.borrow::<u32>().unwrap();
        assert!(matches!(world.borrow_mut::<u64>(), Err(_)));
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        world.new_static_ecs_type_id::<u64>();
        let e1 = world.spawn().insert(10_u32).id();
        let e2 = world.spawn().insert(12_u32).id();
        let u32s = &*world.borrow::<u32>().unwrap();
        let u64s = &*world.borrow::<u64>().unwrap();
        let mut iter = ColumnIterator::new((WithEntities, Maybe(u64s), u32s), &world);
        assert_eq!(iter.next(), Some((e1, None, &10_u32)));
        assert_eq!(iter.next(), Some((e2, None, &12_u32)));
        assert_eq!(iter.next(), None);
    }
}
