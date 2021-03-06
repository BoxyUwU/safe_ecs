#![cfg_attr(not(test), forbid(unsafe_code))]
#![feature(map_try_insert, type_alias_impl_trait, generic_associated_types)]

mod commands;
mod dynamic_storage;
mod entities;
mod query;
mod scope;
mod system;
mod world;

pub use commands::{Command, CommandBuffer, Commands, CommandsWithEntity};
pub use entities::Entity;
pub use query::{DynQueryParam, DynQueryParamKind, Maybe, Query, QueryIter};
pub use safe_ecs_derive::Component;
pub use scope::Scope;
pub use system::{Access, System, SystemParam, ToSystem};
pub use world::{Component, EcsTypeId, EntityBuilder, World};

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

#[cfg(test)]
mod test_component_impls {
    use crate::Component;
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
