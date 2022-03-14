#![cfg_attr(not(test), forbid(unsafe_code))]
#![feature(
    let_else,
    map_try_insert,
    type_alias_impl_trait,
    generic_associated_types
)]

mod column_join;
mod dynamic_storage;
mod entities;
mod static_columns;
mod world;

pub use column_join::{ColumnIterator, ColumnLocks, Joinable, Maybe, Unsatisfied, WithEntities};
pub use entities::Entity;
pub use safe_ecs_derive::Component;
pub use static_columns::StaticColumns;
pub use world::{Component, DynamicColumns, EcsTypeId, EntityBuilder, World, WorldId};

pub mod errors {
    #[derive(Debug, Copy, Clone)]
    pub struct WorldBorrowError(pub &'static str);
}

use std::marker::PhantomData;
use std::mem::MaybeUninit;
pub struct LtPtr<'a>(PhantomData<&'a ()>, pub *const [MaybeUninit<u8>]);
pub struct LtPtrMut<'a>(PhantomData<&'a mut ()>, pub *mut [MaybeUninit<u8>]);
pub struct LtPtrWriteOnly<'a>(PhantomData<&'a mut ()>, pub *mut [MaybeUninit<u8>]);
pub struct LtPtrOwn<'a>(PhantomData<&'a ()>, pub *const [MaybeUninit<u8>]);

pub(crate) fn get_two_mut<T>(vec: &mut [T], idx_1: usize, idx_2: usize) -> (&mut T, &mut T) {
    use std::cmp::Ordering;
    match idx_1.cmp(&idx_2) {
        Ordering::Less => {
            let (left, right) = vec.split_at_mut(idx_2);
            (&mut left[idx_1], &mut right[0])
        }
        Ordering::Greater => {
            let (left, right) = vec.split_at_mut(idx_1);
            (&mut right[0], &mut left[idx_2])
        }
        Ordering::Equal => {
            panic!("")
        }
    }
}

#[cfg(test)]
mod test_component_impls {
    use crate::Component;
    impl<T: Component> Component for &T {}
    impl Component for bool {}
    impl Component for u8 {}
    impl Component for i8 {}
    impl Component for u16 {}
    impl Component for i16 {}
    impl Component for u32 {}
    impl Component for i32 {}
    impl Component for u64 {}
    impl Component for i64 {}
    impl Component for usize {}
    impl Component for isize {}
    impl Component for u128 {}
    impl Component for i128 {}
}

#[cfg(test)]
#[test]
fn derive_macro_works() {
    #[derive(Component)]
    struct Bar;

    fn foo<T: Component>() {}
    foo::<Bar>();
}
