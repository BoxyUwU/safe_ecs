use crate::{
    sealed,
    world::{Archetype, Storage},
    Component, World,
};
use std::{any::TypeId, cell};

pub trait QueryParam: sealed::Sealed {
    type Lock<'a>;
    type LockBorrow<'a>;
    type Item<'a>;
    type ItemIter<'a>;
    fn lock_from_world(world: &World) -> Self::Lock<'_>;
    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a>;
    fn archetype_matches(archetype: &Archetype) -> bool;
    fn item_iter_from_archetype<'a>(
        archetype: &Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a>;
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>>;
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
        archetype: &Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
    ) -> Self::ItemIter<'a> {
        let col = archetype.column_indices[&TypeId::of::<T>()];
        lock_borrow[col].as_vec::<T>().unwrap().iter()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
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
        archetype: &Archetype,
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
}

macro_rules! query_param_tuple_impl {
    ($($T:ident)+) => {
        impl<$($T: sealed::Sealed),+> sealed::Sealed for ($($T,)+) {}
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
            fn item_iter_from_archetype<'a>(archetype: &Archetype, lock_borrow: &mut Self::LockBorrow<'a>) -> Self::ItemIter<'a> {
                let ($($T,)+) = lock_borrow;
                ($($T::item_iter_from_archetype(archetype, $T),)+)
            }

            #[allow(non_snake_case)]
            fn advance_iter<'a>(iters: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
                let ($($T,)+) = iters;
                Some(($($T::advance_iter($T)?,)+))
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

pub struct QueryBorrows<'a, Q: QueryParam>(pub(crate) &'a World, pub(crate) Q::Lock<'a>);
impl<'b, Q: QueryParam> QueryBorrows<'b, Q> {
    fn iter_mut(&mut self) -> QueryIter<'_, 'b, Q> {
        QueryIter::new(self)
    }
}
impl<'a, 'b: 'a, Q: QueryParam> IntoIterator for &'a mut QueryBorrows<'b, Q> {
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
    fn new(borrows: &'a mut QueryBorrows<'b, Q>) -> Self {
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