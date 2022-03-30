#![feature(generic_associated_types, type_alias_impl_trait)]

use core::mem::MaybeUninit;
use safe_ecs::{Archetype, Columns, ColumnsApi, EcsTypeId, Entity, IterableColumns, World};
use std::{
    alloc::Layout,
    any::Any,
    cell::{Ref, RefMut},
};

mod sealed {
    use std::mem::MaybeUninit;

    pub trait AlignedBytes: Copy + 'static {
        type Iter<'a>: Iterator<Item = Self> + 'a;
        fn new_from_iter(data: &[MaybeUninit<u8>]) -> Self::Iter<'_>;
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
    fn swap_remove_to(&mut self, other: &mut dyn AlignedBytesVec, entity_idx: usize);
    fn swap_remove_drop(&mut self, entity_idx: usize);
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

    fn swap_remove_to(&mut self, other: &mut dyn AlignedBytesVec, entity_idx: usize) {
        let other = other.as_any_mut().downcast_mut::<Vec<T>>().unwrap();
        let size = std::mem::size_of::<T>();
        for idx in (size * entity_idx)..(size * (entity_idx + 1)) {
            other.push(self[idx]);
        }
        for idx in ((size * entity_idx)..(size * (entity_idx + 1))).rev() {
            self.swap_remove(idx);
        }
    }

    fn swap_remove_drop(&mut self, entity_idx: usize) {
        let size = std::mem::size_of::<T>();
        for idx in ((size * entity_idx)..(size * (entity_idx + 1))).rev() {
            self.swap_remove(idx);
        }
    }

    fn as_byte_slice(&self) -> &[MaybeUninit<u8>] {
        let len = self.len();
        let this_ptr = self.as_slice() as *const [T] as *const T as *const MaybeUninit<u8>;
        unsafe { std::slice::from_raw_parts(this_ptr, std::mem::size_of::<T>() * len) }
    }

    fn as_byte_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        let len = self.len();
        let this_ptr = self.as_mut_slice() as *mut [T] as *mut T as *mut MaybeUninit<u8>;
        unsafe { std::slice::from_raw_parts_mut(this_ptr, std::mem::size_of::<T>() * len) }
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

    type Remove = ();
    type Get = [MaybeUninit<u8>];

    fn get_component<'a>(
        this: Ref<'a, Self>,
        world: &World,
        id: EcsTypeId,
        entity: Entity,
    ) -> Option<Ref<'a, [MaybeUninit<u8>]>> {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;
        let size = this.layout.size();
        Some(Ref::map(this, |this| {
            &this.buf[column_idx].as_byte_slice()[(entity_idx * size)..((entity_idx + 1) * size)]
        }))
    }

    fn get_component_mut<'a>(
        this: RefMut<'a, Self>,
        world: &World,
        id: EcsTypeId,
        entity: Entity,
    ) -> Option<RefMut<'a, [MaybeUninit<u8>]>> {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id)?;
        let size = this.layout.size();
        Some(RefMut::map(this, |this| {
            &mut this.buf[column_idx].as_byte_slice_mut()
                [(entity_idx * size)..((entity_idx + 1) * size)]
        }))
    }

    fn insert_overwrite<'a>(
        mut overwrite: RefMut<'_, [MaybeUninit<u8>]>,
        data: &'a [MaybeUninit<u8>],
    ) -> Self::Remove
    where
        Self: 'a,
    {
        assert_eq!(overwrite.len(), data.len());
        for overwrite in &mut *overwrite {
            for data in data {
                *overwrite = *data;
            }
        }
    }

    fn insert_component<'a, 'b>(
        mut this: RefMut<'a, Self>,
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
        this.buf[column_idx].push(data);
    }

    fn remove_component<'a>(
        mut this: RefMut<'a, Self>,
        world: &mut World,
        id: EcsTypeId,
        entity: Entity,
    ) {
        let archetype_id = world.entity_meta(entity).unwrap().archetype;
        let archetype = world.get_archetype(archetype_id);
        let entity_idx = archetype.get_entity_idx(entity).unwrap();
        let column_idx = archetype.column_index(id).unwrap();
        this.buf[column_idx].swap_remove_drop(entity_idx);
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
        (&mut **old_col).swap_remove_to(&mut **new_col, entity_idx);
    }

    fn swap_remove_drop(&mut self, col: usize, entity_idx: usize) {
        self.buf[col].swap_remove_drop(entity_idx);
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
