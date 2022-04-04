#![feature(generic_associated_types, type_alias_impl_trait)]

use core::mem::MaybeUninit;
use safe_ecs::{Archetype, Columns, ColumnsApi, EcsTypeId, Entity, IterableColumns, World};
use std::{alloc::Layout, any::Any};

mod sealed {
    use std::mem::MaybeUninit;

    pub trait AlignedBytes: Copy + 'static {
        type Iter<'a>: Iterator<Item = Self> + 'a;
        fn new_from_iter(data: &[MaybeUninit<u8>]) -> Self::Iter<'_>;
        fn slice_to_bytes(data: &[Self]) -> &[MaybeUninit<u8>];
        fn slice_to_bytes_mut(data: &mut [Self]) -> &mut [MaybeUninit<u8>];
    }
}
use sealed::AlignedBytes;
macro_rules! aligned_bytes_type_defs {
    ($($T:ident $A:literal)*) => {
        $(
            #[repr(C, align($A))]
            #[derive(Copy, Clone)]
            pub struct $T([MaybeUninit<u8>; $A]);

            impl $T {
                pub fn new() -> Self {
                    Self([MaybeUninit::uninit(); $A])
                }
            }

            impl AlignedBytes for $T {
                type Iter<'a> = impl Iterator<Item = Self> + 'a;
                fn new_from_iter(data: &[MaybeUninit<u8>]) -> Self::Iter<'_> {
                    data
                        .chunks_exact(std::mem::size_of::<$T>())
                        .map(|data| $T(data.try_into().unwrap()))
                }
                fn slice_to_bytes(data: &[Self]) -> &[MaybeUninit<u8>] {
                    let len = data.len();
                    let this_ptr = data as *const [Self] as *const Self as *const MaybeUninit<u8>;
                    unsafe { std::slice::from_raw_parts(this_ptr, std::mem::size_of::<Self>() * len) }
                }
                fn slice_to_bytes_mut(data: &mut [Self]) -> &mut [MaybeUninit<u8>] {
                    let len = data.len();
                    let this_ptr = data as *mut [Self] as *mut Self as *mut MaybeUninit<u8>;
                    unsafe { std::slice::from_raw_parts_mut(this_ptr, std::mem::size_of::<Self>() * len) }
                }
            }
        )*

        pub(crate) fn new_dynamic_table(layout: Layout) -> DynamicTable {
            match layout.align() {
                $(
                    $A => DynamicTable { buf: vec![Box::new(Vec::<$T>::new())], layout, },
                )*
                _ => unreachable!(),
            }
        }
    };
}

aligned_bytes_type_defs! {
    AlignedBytes1 1
    AlignedBytes2 2
    AlignedBytes4 4
    AlignedBytes8 8
    AlignedBytes16 16
    AlignedBytes32 32
    AlignedBytes64 64
    AlignedBytes128 128
    AlignedBytes256 256
    AlignedBytes512 512
    AlignedBytes1024 1024
    AlignedBytes2048 2048
    AlignedBytes4096 4096
    AlignedBytes8192 8192
    AlignedBytes16384 16384
    AlignedBytes32768 32768
    AlignedBytes65536 65536
    AlignedBytes131072 131072
    AlignedBytes262144 262144
    AlignedBytes524288 524288
    AlignedBytes1048576 1048576
    AlignedBytes2097152 2097152
    AlignedBytes4194304 4194304
    AlignedBytes8388608 8388608
    AlignedBytes16777216 16777216
    AlignedBytes33554432 33554432
    AlignedBytes67108864 67108864
    AlignedBytes134217728 134217728
    AlignedBytes268435456 268435456
    AlignedBytes536870912 536870912
}

pub trait AlignedBytesVec {
    fn new(&self) -> Box<dyn AlignedBytesVec>;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn swap_remove_to(&mut self, other: &mut dyn AlignedBytesVec, elements: std::ops::Range<usize>);
    fn swap_remove_drop(&mut self, elements: std::ops::Range<usize>);
    fn as_byte_slice(&self) -> &[MaybeUninit<u8>];
    fn as_byte_slice_mut(&mut self) -> &mut [MaybeUninit<u8>];
    fn push(&mut self, data: &[MaybeUninit<u8>]);
}
impl<T: AlignedBytes> AlignedBytesVec for Vec<T> {
    fn new(&self) -> Box<dyn AlignedBytesVec> {
        Box::new(Vec::<T>::new())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn swap_remove_to(
        &mut self,
        other: &mut dyn AlignedBytesVec,
        elements: std::ops::Range<usize>,
    ) {
        let other = other.as_any_mut().downcast_mut::<Vec<T>>().unwrap();
        for idx in elements.clone() {
            other.push(self[idx]);
        }
        for idx in elements.rev() {
            self.swap_remove(idx);
        }
    }

    fn swap_remove_drop(&mut self, elements: std::ops::Range<usize>) {
        for idx in elements.rev() {
            self.swap_remove(idx);
        }
    }

    fn as_byte_slice(&self) -> &[MaybeUninit<u8>] {
        T::slice_to_bytes(self.as_slice())
    }

    fn as_byte_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        T::slice_to_bytes_mut(self.as_mut_slice())
    }

    fn push(&mut self, data: &[MaybeUninit<u8>]) {
        for data in T::new_from_iter(data) {
            self.push(data);
        }
    }
}

pub struct DynamicTable {
    buf: Vec<Box<dyn AlignedBytesVec>>,
    layout: Layout,
}
impl DynamicTable {
    pub fn new(layout: Layout) -> Self {
        new_dynamic_table(layout)
    }
}

impl ColumnsApi for DynamicTable {
    type Insert<'a> = &'a [MaybeUninit<u8>]
    where
        Self: 'a;

    // fixme should return `&'a [MaybeUninit<u8>]`
    type Remove = ();
    type Get = [MaybeUninit<u8>];

    fn get_component<'a>(
        &'a self,
        world: &World,
        id: EcsTypeId,
        entity: Entity,
    ) -> Option<&'a [MaybeUninit<u8>]> {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;

        Some(
            &self.buf[column_idx].as_byte_slice()
                [(entity_idx * self.layout.size())..((entity_idx + 1) * self.layout.size())],
        )
    }

    fn get_component_mut<'a>(
        &'a mut self,
        world: &World,
        id: EcsTypeId,
        entity: Entity,
    ) -> Option<&'a mut [MaybeUninit<u8>]> {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;

        Some(
            &mut self.buf[column_idx].as_byte_slice_mut()
                [(entity_idx * self.layout.size())..((entity_idx + 1) * self.layout.size())],
        )
    }

    fn insert_overwrite<'a>(
        overwrite: &mut [MaybeUninit<u8>],
        data: &'a [MaybeUninit<u8>],
    ) -> Self::Remove
    where
        Self: 'a,
    {
        assert_eq!(overwrite.len(), data.len());
        for (overwrite, with) in overwrite.iter_mut().zip(data.iter()) {
            *overwrite = *with;
        }
    }

    fn insert_component<'a, 'b>(
        &'a mut self,
        world: &mut World,
        id: EcsTypeId,
        entity: Entity,
        data: &'b [MaybeUninit<u8>],
    ) where
        Self: 'b,
    {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let column_idx = archetype.column_index(id).unwrap();
        self.buf[column_idx].push(data);
    }

    fn remove_component<'a>(&'a mut self, world: &mut World, id: EcsTypeId, entity: Entity) {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id).unwrap();

        let chunks_per_component = self.layout.size() / self.layout.align();
        let component_chunks =
            (entity_idx * chunks_per_component)..((entity_idx + 1) * chunks_per_component);

        self.buf[column_idx].swap_remove_drop(component_chunks);
    }
}

impl Columns for DynamicTable {
    fn push_empty_column(&mut self) -> usize {
        let new = self.buf[0].new();
        self.buf.push(new);
        self.buf.len() - 1
    }

    fn swap_remove_to(&mut self, old_col: usize, new_col: usize, entity_idx: usize) {
        let (old_col, new_col) = safe_ecs::get_two_mut(&mut self.buf[..], old_col, new_col);
        let chunks_per_component = self.layout.size() / self.layout.align();
        let component_chunks =
            (entity_idx * chunks_per_component)..((entity_idx + 1) * chunks_per_component);
        (&mut **old_col).swap_remove_to(&mut **new_col, component_chunks);
    }

    fn swap_remove_drop(&mut self, col: usize, entity_idx: usize) {
        let chunks_per_component = self.layout.size() / self.layout.align();
        let component_chunks =
            (entity_idx * chunks_per_component)..((entity_idx + 1) * chunks_per_component);
        self.buf[col].swap_remove_drop(component_chunks);
    }
}
impl<'a> IterableColumns for &'a DynamicTable {
    type Item = &'a [MaybeUninit<u8>];
    type IterState = (usize, EcsTypeId, &'a [Box<dyn AlignedBytesVec>]);
    type ArchetypeState = std::slice::ChunksExact<'a, MaybeUninit<u8>>;

    fn make_iter_state(id: safe_ecs::EcsTypeId, locks: Self) -> Self::IterState {
        (locks.layout.size(), id, &locks.buf[..])
    }

    fn make_archetype_state<'lock>(
        (size, id, state): &mut Self::IterState,
        archetype: &'lock Archetype,
    ) -> Self::ArchetypeState
    where
        Self: 'lock,
    {
        let col = archetype.column_index(*id).unwrap();
        state[col].as_byte_slice().chunks_exact(*size)
    }

    fn item_of_entity(iter: &mut Self::ArchetypeState) -> Option<Self::Item> {
        iter.next()
    }
}
impl<'a> IterableColumns for &'a mut DynamicTable {
    type Item = &'a mut [MaybeUninit<u8>];
    type IterState = (usize, EcsTypeId, usize, &'a mut [Box<dyn AlignedBytesVec>]);
    type ArchetypeState = std::slice::ChunksExactMut<'a, MaybeUninit<u8>>;

    fn make_iter_state(id: safe_ecs::EcsTypeId, locks: Self) -> Self::IterState {
        (locks.layout.size(), id, 0, &mut locks.buf[..])
    }

    fn make_archetype_state<'lock>(
        (size, ecs_type_id, num_chopped_off, lock_borrow): &mut Self::IterState,
        archetype: &'lock Archetype,
    ) -> Self::ArchetypeState
    where
        Self: 'lock,
    {
        let col = archetype.column_index(*ecs_type_id).unwrap();
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of
            .last_mut()
            .unwrap()
            .as_byte_slice_mut()
            .chunks_exact_mut(*size)
    }

    fn item_of_entity(iter: &mut Self::ArchetypeState) -> Option<Self::Item> {
        iter.next()
    }
}

#[cfg(test)]
fn assert_bytes<T: PartialEq + std::fmt::Debug>(data: &[MaybeUninit<u8>], asserted: T) {
    let data = unsafe { &*(data as *const [MaybeUninit<u8>]).cast::<T>() };
    assert_eq!(data, &asserted);
}

#[cfg(test)]
fn as_bytes<T>(data: &T) -> &[MaybeUninit<u8>] {
    unsafe {
        std::slice::from_raw_parts(
            data as *const T as *const MaybeUninit<u8>,
            std::mem::size_of::<T>(),
        )
    }
}

#[cfg(test)]
fn unas_bytes<T>(data: &[MaybeUninit<u8>]) -> &T {
    unsafe { &*(data as *const [MaybeUninit<u8>] as *const T) }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use safe_ecs::*;

    #[test]
    fn is_miri() {
        #[cfg(not(miri))]
        panic!("")
    }

    trait UnwrapNone {
        fn unwrap_none(self);
    }

    impl<T> UnwrapNone for Option<T> {
        fn unwrap_none(self) {
            match self {
                Some(_) => panic!("expected `None` found `Some(_)`"),
                None => (),
            }
        }
    }

    #[test]
    fn has_component() {
        let mut world = World::new();
        let u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        assert_eq!(u32s.has_component(&world, e), false);
    }

    #[test]
    fn basic_insert() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 10_u32);
    }

    #[test]
    fn insert_overwrite() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 10_u32);
        u32s.insert_component(&mut world, e, as_bytes(&12_u32))
            .unwrap();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 12_u32);
    }

    #[test]
    fn insert_archetype_change() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let mut u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        u64s.insert_component(&mut world, e, as_bytes(&12_u64))
            .unwrap_none();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 10_u32);
        u32s.insert_component(&mut world, e, as_bytes(&15_u32))
            .unwrap();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 15_u32);
        assert_bytes(&*u64s.get_component(&world, e).unwrap(), 12_u64);
    }

    #[test]
    #[should_panic = "Unexpected dead entity"]
    fn insert_on_dead() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        world.despawn(e);
        u32s.insert_component(&mut world, e, as_bytes(&12_u32))
            .unwrap_none();
    }

    #[test]
    fn basic_remove() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.remove_component(&mut world, e).unwrap_none();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 10_u32);
        u32s.remove_component(&mut world, e).unwrap();
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn remove_archetype_change() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let mut u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        u64s.insert_component(&mut world, e, as_bytes(&12_u64))
            .unwrap_none();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 10_u32);
        u32s.insert_component(&mut world, e, as_bytes(&15_u32))
            .unwrap();
        assert_bytes(&*u64s.get_component(&world, e).unwrap(), 12_u64);
        u64s.remove_component(&mut world, e).unwrap();
        assert_bytes(&*u32s.get_component(&world, e).unwrap(), 15_u32);
        assert_eq!(u64s.has_component(&world, e), false);
    }

    #[test]
    #[should_panic = "Unexpected dead entity"]
    fn remove_on_dead() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        world.despawn(e);
        u32s.remove_component(&mut world, e).unwrap_none();
    }

    #[test]
    fn get_component() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e = world.spawn().id();
        u32s.insert_component(&mut world, e, as_bytes(&10_u32))
            .unwrap_none();
        assert_bytes(&*u32s.get_component_mut(&world, e).unwrap(), 10_u32);
    }

    #[test]
    fn for_loop() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32));
        for (entity, data) in ColumnLocks::new((WithEntities, &u32s), &world).into_iter() {
            assert_eq!(entity, e1);
            assert_bytes(data, 10_u32);
            return;
        }
        unreachable!()
    }

    #[test]
    fn simple_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let mut u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));
        let mut u128s = world.new_handle(DynamicTable::new(Layout::new::<u128>()));

        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32))
            .unwrap_none();
        u64s.insert_component(&mut world, e1, as_bytes(&12_u64))
            .unwrap_none();

        let e2 = world.spawn().id();
        u64s.insert_component(&mut world, e2, as_bytes(&13_u64))
            .unwrap_none();
        u128s
            .insert_component(&mut world, e2, as_bytes(&9_u128))
            .unwrap_none();

        let mut locks = ColumnLocks::new(&u64s, &world);
        let returned = locks
            .into_iter()
            .map(|data1| unas_bytes::<u64>(data1))
            .collect::<Vec<_>>();
        assert_eq!(returned, [&12_u64, &13_u64]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let mut u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));
        let mut u128s = world.new_handle(DynamicTable::new(Layout::new::<u128>()));

        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32))
            .unwrap_none();
        u64s.insert_component(&mut world, e1, as_bytes(&12_u64))
            .unwrap_none();

        let e2 = world.spawn().id();
        u64s.insert_component(&mut world, e2, as_bytes(&13_u64))
            .unwrap_none();
        u128s
            .insert_component(&mut world, e2, as_bytes(&9_u128))
            .unwrap_none();

        let mut locks = ColumnLocks::new((WithEntities, &u32s, &u64s), &world);
        let returned = locks
            .into_iter()
            .map(|(entity, data1, data2)| {
                (entity, unas_bytes::<u32>(data1), unas_bytes::<u64>(data2))
            })
            .collect::<Vec<_>>();
        assert_eq!(returned, [(e1, &10_u32, &12_u64)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let mut u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));
        let mut u128s = world.new_handle(DynamicTable::new(Layout::new::<u128>()));

        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32))
            .unwrap_none();
        u64s.insert_component(&mut world, e1, as_bytes(&12_u64))
            .unwrap_none();

        let e2 = world.spawn().id();
        u64s.insert_component(&mut world, e2, as_bytes(&13_u64))
            .unwrap_none();
        u128s
            .insert_component(&mut world, e2, as_bytes(&9_u128))
            .unwrap_none();

        let mut locks =
            ColumnLocks::new((WithEntities, Maybe(&u32s), &u64s, Maybe(&u128s)), &world);
        let returned = locks
            .into_iter()
            .map(|(entity, data1, data2, data3)| {
                (
                    entity,
                    data1.map(unas_bytes::<u32>),
                    unas_bytes::<u64>(data2),
                    data3.map(unas_bytes::<u128>),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            returned,
            [
                (e1, Some(&10_u32), &12_u64, None),
                (e2, None, &13_u64, Some(&9_u128))
            ],
        )
    }

    #[test]
    fn query_with_despawned() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32))
            .unwrap_none();
        world.despawn(e1);

        let mut locks = ColumnLocks::new(&u32s, &world);
        let mut iter = locks.into_iter();
        assert!(iter.next().is_none());
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        let mut u32s = world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        let u64s = world.new_handle(DynamicTable::new(Layout::new::<u64>()));

        let e1 = world.spawn().id();
        u32s.insert_component(&mut world, e1, as_bytes(&10_u32))
            .unwrap_none();

        let e2 = world.spawn().id();
        u32s.insert_component(&mut world, e2, as_bytes(&12_u32))
            .unwrap_none();

        let mut locks = ColumnLocks::new((WithEntities, Maybe(&u64s), &u32s), &world);
        let returned = locks
            .into_iter()
            .map(|(entity, data1, data2)| {
                (
                    entity,
                    data1.map(unas_bytes::<u64>),
                    unas_bytes::<u32>(data2),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(returned, [(e1, None, &10_u32), (e2, None, &12_u32)]);
    }
}

#[cfg(test)]
mod mismatched_world_id_tests {
    use crate::*;
    use safe_ecs::*;

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn ref_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        ColumnLocks::new(&other_u32s, &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn mut_join() {
        let world = World::new();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        ColumnLocks::new(&mut other_u32s, &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn maybe_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        ColumnLocks::new(Maybe(&other_u32s), &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn unsatisfied_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        ColumnLocks::new(Unsatisfied(&other_u32s), &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn multi_join() {
        let world = World::new();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        ColumnLocks::new((WithEntities, &other_u32s), &world);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn remove_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        other_u32s.remove_component(&mut world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn insert_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        other_u32s.insert_component(&mut world, e, as_bytes(&10_u32));
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn has_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        other_u32s.has_component(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component_mut() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let mut other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        other_u32s.get_component_mut(&world, e);
    }

    #[test]
    #[should_panic = "[Mismatched WorldIds]:"]
    fn get_component() {
        let mut world = World::new();
        let e = world.spawn().id();
        let mut other_world = World::new();
        let other_u32s = other_world.new_handle(DynamicTable::new(Layout::new::<u32>()));
        other_u32s.get_component(&world, e);
    }
}
