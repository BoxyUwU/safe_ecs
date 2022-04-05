use crate::world::Archetype;
use crate::WorldId;
use crate::{Entity, World};

pub struct ColumnIterator<'a, C: Joinable + 'a> {
    ids: C::Ids,
    archetypes: std::slice::Iter<'a, Archetype>,
    column_iter: Option<C::ArchetypeState<'a>>,
    joined: C::IterState<'a>,
}

impl<'a, C: Joinable + 'a> ColumnIterator<'a, C> {
    pub fn new(joined: C, world: &'a World) -> Self {
        C::assert_world_id(&joined, world.id());
        let ids = C::make_ids(&joined, world);
        Self {
            ids,
            archetypes: world.archetypes.iter(),
            column_iter: None,
            joined: C::make_iter_state(joined, world),
        }
    }
}

impl<'a, C: Joinable + 'a> Iterator for ColumnIterator<'a, C> {
    type Item = C::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match &mut self.column_iter {
                Some(iter) => match C::make_item(iter) {
                    Some(v) => return Some(v),
                    None => {
                        self.column_iter = None;
                        continue;
                    }
                },
                None => {
                    let archetype = self
                        .archetypes
                        .find(|archetype| C::archetype_matches(&self.ids, archetype))?;
                    let iter = C::make_archetype_state(&mut self.joined, archetype);
                    self.column_iter = Some(iter);
                    continue;
                }
            }
        }
    }
}

//~ joinable impls

/// This trait is also implemented for tuples up to length 8 where all elements implement this trait
pub trait Joinable {
    type Ids: Copy;

    type IterState<'world>
    where
        Self: 'world;

    type ArchetypeState<'world>
    where
        Self: 'world;

    type Item<'world>
    where
        Self: 'world;

    fn assert_world_id(&self, world_id: WorldId);
    fn make_ids(&self, world: &World) -> Self::Ids;

    fn make_iter_state<'world>(self, world: &'world World) -> Self::IterState<'world>
    where
        Self: 'world;

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool;
    fn make_archetype_state<'world>(
        state: &mut Self::IterState<'world>,
        archetype: &'world Archetype,
    ) -> Self::ArchetypeState<'world>
    where
        Self: 'world;

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world;
}

pub struct WithEntities;
impl Joinable for WithEntities {
    type Ids = ();

    type IterState<'lock> = ()
    where
        Self: 'lock;

    type Item<'lock> = Entity
    where
        Self: 'lock;

    type ArchetypeState<'lock> = std::slice::Iter<'lock, Entity>
    where
        Self: 'lock;

    fn make_ids(&self, _: &World) -> Self::Ids {}

    fn make_iter_state<'world>(self, _: &'world World) -> Self::IterState<'world>
    where
        Self: 'world,
    {
    }

    fn make_archetype_state<'world>(
        _: &mut Self::IterState<'world>,
        archetype: &'world Archetype,
    ) -> Self::ArchetypeState<'world>
    where
        Self: 'world,
    {
        archetype.entities.iter()
    }

    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next().copied()
    }

    fn assert_world_id(&self, _: WorldId) {}
}

pub struct Maybe<J: Joinable>(pub J);
pub enum Either<T, U> {
    T(T),
    U(U),
}
impl<J: Joinable> Joinable for Maybe<J> {
    type Ids = ();

    type IterState<'lock> = (J::Ids, J::IterState<'lock>)
    where
        Self: 'lock;

    type Item<'lock> = Option<J::Item<'lock>>
    where
        Self: 'lock;

    type ArchetypeState<'lock> = Either<J::ArchetypeState<'lock>, std::ops::Range<usize>>
    where
        Self: 'lock;

    fn make_ids(&self, _: &World) -> Self::Ids {}

    fn make_iter_state<'world>(self, world: &'world World) -> Self::IterState<'world>
    where
        Self: 'world,
    {
        (
            J::make_ids(&self.0, world),
            J::make_iter_state(self.0, world),
        )
    }

    fn make_archetype_state<'world>(
        (ids, state): &mut Self::IterState<'world>,
        archetype: &'world Archetype,
    ) -> Self::ArchetypeState<'world>
    where
        Self: 'world,
    {
        match J::archetype_matches(ids, archetype) {
            true => Either::T(J::make_archetype_state(state, archetype)),
            false => Either::U(0..archetype.entities.len()),
        }
    }

    fn archetype_matches(_: &Self::Ids, _: &Archetype) -> bool {
        true
    }

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        match iter {
            Either::T(t) => J::make_item(t).map(Some),
            Either::U(u) => u.next().map(|_| None),
        }
    }

    fn assert_world_id(&self, world_id: WorldId) {
        J::assert_world_id(&self.0, world_id)
    }
}

pub struct Unsatisfied<J: Joinable>(pub J);
impl<J: Joinable> Joinable for Unsatisfied<J> {
    type Ids = J::Ids;

    type IterState<'lock> = J::IterState<'lock>
    where
        Self: 'lock;

    type Item<'lock> = ()
    where
        Self: 'lock;

    type ArchetypeState<'lock> = std::ops::Range<usize>
    where
        Self: 'lock;

    fn make_ids(&self, world: &World) -> Self::Ids {
        J::make_ids(&self.0, world)
    }

    fn make_iter_state<'world>(self, world: &'world World) -> Self::IterState<'world>
    where
        Self: 'world,
    {
        J::make_iter_state(self.0, world)
    }

    fn make_archetype_state<'world>(
        _: &mut Self::IterState<'world>,
        archetype: &'world Archetype,
    ) -> Self::ArchetypeState<'world>
    where
        Self: 'world,
    {
        0..archetype.entities.len()
    }

    fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
        J::archetype_matches(ids, archetype) == false
    }

    fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
    where
        Self: 'world,
    {
        iter.next().map(|_| ())
    }

    fn assert_world_id(&self, world_id: WorldId) {
        J::assert_world_id(&self.0, world_id)
    }
}

macro_rules! tuple_impls_joinable {
    ($($T:ident)*) => {
        #[doc(hidden)]
        #[allow(unused_parens)]
        #[allow(non_snake_case)]
        impl<$($T: Joinable),*> Joinable for ($($T,)*) {
            type Ids = ($($T::Ids,)*);

            type IterState<'lock> = ($($T::IterState<'lock>,)*)
            where
                Self: 'lock;

            type Item<'lock> = ($($T::Item<'lock>,)*)
            where
                Self: 'lock;

            type ArchetypeState<'lock> = ($($T::ArchetypeState<'lock>,)*)
            where
                Self: 'lock;

            fn make_ids(&self, world: &World) -> Self::Ids {
                let ($($T,)*) = self;
                ($($T::make_ids($T, world),)*)
            }
            fn make_iter_state<'world>(self, world: &'world World) -> Self::IterState<'world>
            where
                Self: 'world {
                    let ($($T,)*) = self;
                    ($($T::make_iter_state($T, world),)*)
                }
            fn make_archetype_state<'world>(
                state: &mut Self::IterState<'world>,
                archetype: &'world Archetype,
            ) -> Self::ArchetypeState<'world>
            where
                Self: 'world {
                    let ($($T,)*) = state;
                    ($($T::make_archetype_state($T, archetype),)*)
                }
            fn archetype_matches(ids: &Self::Ids, archetype: &Archetype) -> bool {
                let ($($T,)*) = ids;
                true $(&& $T::archetype_matches($T, archetype))*
            }
            fn make_item<'world>(iter: &mut Self::ArchetypeState<'world>) -> Option<Self::Item<'world>>
            where
                Self: 'world {
                    let ($($T,)*) = iter;
                    Some(($($T::make_item($T)?,)*))
                }
            fn assert_world_id(&self, world_id: WorldId) {
                let ($($T,)*) = self;
                $($T::assert_world_id($T, world_id);)*
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
