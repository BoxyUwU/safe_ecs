#![cfg_attr(not(test), forbid(unsafe_code))]
#![feature(type_alias_impl_trait, generic_associated_types)]

mod column_join;
mod entities;
mod static_columns;
mod world;

pub use column_join::{ColumnIterator, ColumnLocks, Joinable, Maybe, Unsatisfied, WithEntities};
pub use entities::Entity;
pub use static_columns::Table;
pub use world::{
    Archetype, Columns, ColumnsApi, EcsTypeId, EntityBuilder, Handle, IterableColumns, World,
    WorldId,
};

pub fn get_two_mut<T>(vec: &mut [T], idx_1: usize, idx_2: usize) -> (&mut T, &mut T) {
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
