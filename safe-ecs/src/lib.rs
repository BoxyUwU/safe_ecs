#![forbid(unsafe_code)]
#![feature(type_alias_impl_trait, generic_associated_types)]

mod query;
mod system;
mod world;

pub use query::{Maybe, QueryBorrows, QueryIter};
pub use safe_ecs_derive::Component;
pub use world::{Component, Entity, EntityBuilder, World};

pub(crate) mod sealed {
    pub trait Sealed {}
}

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
