use std::cell::{Ref, RefMut};

use crate::static_columns::StaticColumnsInner;
use crate::world::Archetype;
use crate::StaticColumns;
use crate::{Component, EcsTypeId, Entity, World};

pub struct ColumnIterator<'lock, 'world: 'lock, C: Joinable + 'lock> {
    archetype_iter: ArchetypeIter<'world, 'lock, C>,
    state: C::State<'lock>,
    column_iter: Option<C::ItemIter<'lock>>,
}

pub struct ColumnLocks<'world, C: Joinable + 'world> {
    ids: C::Ids,
    world: &'world World,
    locks: C::Locks<'world>,
}

type ArchetypeIter<'world: 'lock, 'lock, C: Joinable + 'lock> =
    impl Iterator<Item = &'world Archetype> + 'lock;
impl<'world, C: Joinable + 'world> ColumnLocks<'world, C> {
    pub fn new(borrows: C, world: &'world World) -> Self {
        let ids = C::make_ids(&borrows, world);
        let locks = C::make_locks(borrows, world);
        Self { ids, locks, world }
    }
}

impl<'lock, 'world, C: Joinable> ColumnIterator<'lock, 'world, C> {
    pub fn new(locks: &'lock mut ColumnLocks<'world, C>) -> Self {
        let state = C::make_state(&mut locks.locks);

        fn defining_use<'world: 'lock, 'lock, C: Joinable + 'lock>(
            world: &'world World,
            ids: C::Ids,
        ) -> ArchetypeIter<'world, 'lock, C> {
            world
                .archetypes
                .iter()
                .filter(move |archetype| C::archetype_matches(&ids, archetype))
        }
        ColumnIterator {
            archetype_iter: defining_use(locks.world, locks.ids),
            state,
            column_iter: None,
        }
    }
}

impl<'lock, 'world: 'lock, C: Joinable + 'lock> Iterator for ColumnIterator<'lock, 'world, C> {
    type Item = C::Item<'lock>;

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

impl<'lock, 'world: 'lock, C: Joinable + 'lock> IntoIterator for &'lock mut ColumnLocks<'world, C> {
    type Item = C::Item<'lock>;
    type IntoIter = ColumnIterator<'lock, 'world, C>;

    fn into_iter(self) -> Self::IntoIter {
        ColumnIterator::new(self)
    }
}

//~ joinable impls

/// This trait is also implemented for tuples up to length 8 where all elements implement this trait
pub trait Joinable {
    type Ids: Copy;

    type Locks<'world>
    where
        Self: 'world;

    type State<'lock>
    where
        Self: 'lock;

    type Item<'lock>
    where
        Self: 'lock;

    type ItemIter<'lock>
    where
        Self: 'lock;

    fn make_ids(&self, world: &World) -> Self::Ids;
    fn make_locks<'world>(self, world: &'world World) -> Self::Locks<'world>
    where
        Self: 'world;
    fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'lock + 'world;
    fn iter_from_archetype<'world>(
        state: &mut Self::State<'world>,
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
    where
        Self: 'world;
    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool;
    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world;
}

impl<'a, T: Component> Joinable for &'a StaticColumns<T> {
    type Ids = EcsTypeId;
    type Locks<'world>
    where
        Self: 'world,
    = (EcsTypeId, Ref<'world, StaticColumnsInner<T>>);

    type State<'lock>
    where
        Self: 'lock,
    = (EcsTypeId, &'lock [Vec<T>]);

    type Item<'lock>
    where
        Self: 'lock,
    = &'lock T;

    type ItemIter<'lock>
    where
        Self: 'lock,
    = std::slice::Iter<'lock, T>;

    fn make_ids(&self, _: &World) -> Self::Ids {
        self.id
    }

    fn make_locks<'world>(self, _: &'world World) -> Self::Locks<'world>
    where
        Self: 'world,
    {
        (self.id, self.inner.borrow())
    }

    fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'world + 'lock,
    {
        (locks.0, &locks.1 .0[..])
    }

    fn iter_from_archetype<'world>(
        (id, state): &mut (EcsTypeId, &'world [Vec<T>]),
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
    where
        Self: 'world,
    {
        let col = archetype.column_indices[id];
        let foo = *state;
        foo[col].iter()
    }

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(ids)
    }

    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next()
    }
}

impl<'a, T: Component> Joinable for &'a mut StaticColumns<T> {
    type Ids = EcsTypeId;
    type Locks<'world>
    where
        Self: 'world,
    = (EcsTypeId, RefMut<'world, StaticColumnsInner<T>>);

    type State<'lock>
    where
        Self: 'lock,
    = (EcsTypeId, usize, &'lock mut [Vec<T>]);

    type Item<'lock>
    where
        Self: 'lock,
    = &'lock mut T;

    type ItemIter<'lock>
    where
        Self: 'lock,
    = std::slice::IterMut<'lock, T>;

    fn make_ids(&self, _: &World) -> Self::Ids {
        self.id
    }

    fn make_locks<'world>(self, _: &'world World) -> Self::Locks<'world>
    where
        Self: 'world,
    {
        (self.id, self.inner.borrow_mut())
    }

    fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'world + 'lock,
    {
        (locks.0, 0, &mut locks.1 .0[..])
    }

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        archetype.column_indices.contains_key(ids)
    }

    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next()
    }

    fn iter_from_archetype<'world>(
        (ecs_type_id, num_chopped_off, lock_borrow): &mut Self::State<'world>,
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
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
}

pub struct WithEntities;
impl Joinable for WithEntities {
    type Ids = ();

    type Locks<'world>
    where
        Self: 'world,
    = ();

    type State<'lock>
    where
        Self: 'lock,
    = ();

    type Item<'lock>
    where
        Self: 'lock,
    = Entity;

    type ItemIter<'lock>
    where
        Self: 'lock,
    = std::slice::Iter<'lock, Entity>;

    fn make_ids(&self, _: &World) -> Self::Ids {}

    fn make_locks<'world>(self, _: &'world World) -> Self::Locks<'world>
    where
        Self: 'world,
    {
    }

    fn make_state<'lock, 'world>(_: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'lock + 'world,
    {
    }

    fn iter_from_archetype<'world>(
        _: &mut Self::State<'world>,
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
    where
        Self: 'world,
    {
        archetype.entities.iter()
    }

    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }

    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
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

    type Locks<'world>
    where
        Self: 'world,
    = (J::Ids, J::Locks<'world>);

    type State<'lock>
    where
        Self: 'lock,
    = (J::Ids, J::State<'lock>);

    type Item<'lock>
    where
        Self: 'lock,
    = Option<J::Item<'lock>>;

    type ItemIter<'lock>
    where
        Self: 'lock,
    = Either<J::ItemIter<'lock>, std::ops::Range<usize>>;

    fn make_ids(&self, _: &World) -> Self::Ids {}

    fn make_locks<'world>(self, world: &'world World) -> Self::Locks<'world>
    where
        Self: 'world,
    {
        (J::make_ids(&self.0, world), J::make_locks(self.0, world))
    }

    fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'lock + 'world,
    {
        (locks.0, J::make_state(&mut locks.1))
    }

    fn iter_from_archetype<'world>(
        (ids, state): &mut Self::State<'world>,
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
    where
        Self: 'world,
    {
        match J::archetype_matches(ids, archetype) {
            true => Either::T(J::iter_from_archetype(state, archetype)),
            false => Either::U(0..archetype.entities.len()),
        }
    }

    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }

    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        match iter {
            Either::T(t) => J::advance_iter(t).map(Some),
            Either::U(u) => u.next().map(|_| None),
        }
    }
}

pub struct Unsatisfied<J: Joinable>(J);
impl<J: Joinable> Joinable for Unsatisfied<J> {
    type Ids = J::Ids;

    type Locks<'world>
    where
        Self: 'world,
    = J::Locks<'world>;

    type State<'lock>
    where
        Self: 'lock,
    = J::State<'lock>;

    type Item<'lock>
    where
        Self: 'lock,
    = ();

    type ItemIter<'lock>
    where
        Self: 'lock,
    = std::ops::Range<usize>;

    fn make_ids(&self, world: &World) -> Self::Ids {
        J::make_ids(&self.0, world)
    }

    fn make_locks<'world>(self, world: &'world World) -> Self::Locks<'world>
    where
        Self: 'world,
    {
        J::make_locks(self.0, world)
    }

    fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
    where
        Self: 'lock + 'world,
    {
        J::make_state(locks)
    }

    fn iter_from_archetype<'world>(
        _: &mut Self::State<'world>,
        archetype: &'world Archetype,
    ) -> Self::ItemIter<'world>
    where
        Self: 'world,
    {
        0..archetype.entities.len()
    }

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        J::archetype_matches(ids, archetype) == false
    }

    fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
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

            type Locks<'world>
            where
                Self: 'world = ($($T::Locks<'world>,)*);

            type State<'lock>
            where
                Self: 'lock = ($($T::State<'lock>,)*);

            type Item<'lock>
            where
                Self: 'lock = ($($T::Item<'lock>,)*);

            type ItemIter<'lock>
            where
                Self: 'lock = ($($T::ItemIter<'lock>,)*);

            fn make_ids(&self, world: &World) -> Self::Ids {
                let ($($T,)*) = self;
                ($($T::make_ids($T, world),)*)
            }
            fn make_locks<'world>(self, world: &'world World) -> Self::Locks<'world>
            where
                Self: 'world {
                    let ($($T,)*) = self;
                    ($($T::make_locks($T, world),)*)
                }
            fn make_state<'lock, 'world>(locks: &'lock mut Self::Locks<'world>) -> Self::State<'lock>
            where
                Self: 'lock + 'world {
                    let ($($T,)*) = locks;
                    ($($T::make_state($T),)*)
                }
            fn iter_from_archetype<'world>(
                state: &mut Self::State<'world>,
                archetype: &'world Archetype,
            ) -> Self::ItemIter<'world>
            where
                Self: 'world {
                    let ($($T,)*) = state;
                    ($($T::iter_from_archetype($T, archetype),)*)
                }
            fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
                let ($($T,)*) = ids;
                true $(&& $T::archetype_matches($T, archetype))*
            }
            fn advance_iter<'world>(iter: &mut Self::ItemIter<'world>) -> Option<Self::Item<'world>>
            where
                Self: 'world {
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

#[test]
fn foo() {
    let mut world = World::new();
    #[derive(Debug, Component, Eq, PartialEq)]
    struct Foo(u32);
    let mut u32s = world.new_static_column::<Foo>();
    let e1 = world.spawn().id();
    u32s.insert_component(&mut world, e1, Foo(12_u32));
    let mut locks = ColumnLocks::new(&u32s, &world);
    for _ in &mut locks {}
    let mut iter = ColumnIterator::new(&mut locks);
    assert_eq!(iter.next().unwrap(), &Foo(12_u32));
    assert_eq!(iter.next(), None);
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
