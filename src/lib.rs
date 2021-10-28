#![forbid(unsafe_code)]
#![feature(type_alias_impl_trait, generic_associated_types)]

mod query;
mod world;

pub use world::{Component, Entity, World};

pub(crate) mod sealed {
    pub trait Sealed {}
}
