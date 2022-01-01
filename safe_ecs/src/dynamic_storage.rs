use std::{alloc::Layout, any::Any, mem::MaybeUninit};

use crate::{LtPtr, LtPtrMut, LtPtrOwn, LtPtrWriteOnly};

struct AlignedBytesVec<const A: usize>
where
    (): AlignTo<A>,
{
    inserted_over_space: Vec<<() as AlignTo<A>>::Aligned>,
    buf: Vec<<() as AlignTo<A>>::Aligned>,
    len_elements: usize,
    size: usize,
}

fn index_range_of_element(size: usize, align: usize, idx: usize) -> std::ops::Range<usize> {
    let byte_start_idx = size * idx;
    let byte_end_idx = byte_start_idx + size;
    (byte_start_idx / align)..(byte_end_idx / align)
}

pub trait ErasedBytesVec {
    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_>;
    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_>;
    fn realloc_if_full(&mut self);
    fn empty_of_same_layout(&self) -> Box<dyn ErasedBytesVec>;
    fn swap_remove_move_to(&mut self, other: &mut dyn ErasedBytesVec, idx: usize);
    fn swap_remove(&mut self, idx: usize) -> Option<LtPtrOwn<'_>>;
    fn copy_to_insert_over_space(&mut self, idx: usize) -> (LtPtrOwn<'_>, LtPtrWriteOnly<'_>);
    fn num_elements(&self) -> usize;
    fn incr_len(&mut self);

    fn erased_as_any(&self) -> &dyn Any;
    fn erased_as_any_mut(&mut self) -> &mut dyn Any;

    fn iter(&self) -> Box<dyn Iterator<Item = *mut u8> + '_>;
    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = *mut u8> + '_>;
}
impl<const A: usize> ErasedBytesVec for AlignedBytesVec<A>
where
    (): AlignTo<A>,
{
    fn get_element_ptr(&self, idx: usize) -> LtPtr<'_> {
        let idx = index_range_of_element(self.size, A, idx);
        let ptr = &self.buf[idx] as *const [_] as *const MaybeUninit<u8>;
        let ptr = std::ptr::slice_from_raw_parts(ptr, self.size);
        LtPtr(Default::default(), ptr)
    }

    fn get_element_ptr_mut(&mut self, idx: usize) -> LtPtrMut<'_> {
        let idx = index_range_of_element(self.size, A, idx);
        let ptr = &mut self.buf[idx] as *mut [_] as *mut MaybeUninit<u8>;
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, self.size);
        LtPtrMut(Default::default(), ptr)
    }

    fn realloc_if_full(&mut self) {
        if self.size == 0 {
            return;
        }

        if self.len_elements == (self.buf.len() / self.size) {
            match self.buf.len() {
                0 => self.buf.resize_with(self.size, Default::default),
                n => self.buf.resize_with(n * 2, Default::default),
            }
        }
    }

    fn empty_of_same_layout(&self) -> Box<dyn ErasedBytesVec> {
        Self::new(self.size)
    }

    fn swap_remove_move_to(&mut self, other: &mut dyn ErasedBytesVec, idx: usize) {
        if self.len_elements == 0 {
            panic!("");
        }

        other.realloc_if_full();
        let other = other.unerase_alignement_mut::<A>();
        let src = index_range_of_element(self.size, A, idx);
        let src = &mut self.buf[src];
        let dst = index_range_of_element(other.size, A, other.len_elements);
        let dst = &mut other.buf[dst];

        for (src, dst) in src.into_iter().zip(dst.into_iter()) {
            *dst = *src;
        }

        other.incr_len();

        self.swap_remove(idx);
    }

    fn swap_remove(&mut self, idx: usize) -> Option<LtPtrOwn<'_>> {
        if self.len_elements == 0 {
            panic!("");
        }

        if idx == self.len_elements - 1 {
            self.len_elements -= 1;
            let ptr = self.get_element_ptr_mut(self.len_elements);
            return Some(LtPtrOwn(Default::default(), ptr.1 as *const _));
        }

        let src = index_range_of_element(self.size, A, self.len_elements - 1);
        let dst = index_range_of_element(self.size, A, idx);

        let (dst_slice, src_slice) = self.buf.as_mut_slice().split_at_mut(src.start);
        let src_slice = &mut src_slice[0..self.size];
        let dst_slice = &mut dst_slice[dst];

        for (src, dst) in src_slice.into_iter().zip(dst_slice.into_iter()) {
            std::mem::swap(src, dst);
        }

        self.len_elements -= 1;
        let ptr = self.get_element_ptr_mut(self.len_elements);
        Some(LtPtrOwn(Default::default(), ptr.1 as *const _))
    }

    fn copy_to_insert_over_space(&mut self, idx: usize) -> (LtPtrOwn<'_>, LtPtrWriteOnly<'_>) {
        let src_idx = index_range_of_element(self.size, A, idx);
        let src = &mut self.buf[src_idx];
        let dst = &mut self.inserted_over_space[..];

        for (src, dst) in src.into_iter().zip(dst.into_iter()) {
            *dst = *src;
        }

        let src_ptr = src as *mut [_] as *mut [MaybeUninit<u8>];
        let dest_ptr = dst as *mut [_] as *mut [MaybeUninit<u8>] as *const [MaybeUninit<u8>];
        (
            LtPtrOwn(Default::default(), dest_ptr),
            LtPtrWriteOnly(Default::default(), src_ptr),
        )
    }

    fn num_elements(&self) -> usize {
        self.len_elements
    }

    fn incr_len(&mut self) {
        if self.size == 0 {
            self.len_elements += 1;
        }

        assert!(self.len_elements < (self.buf.len() / self.size));
        self.len_elements += 1;
    }

    fn erased_as_any(&self) -> &dyn Any {
        self
    }

    fn erased_as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn iter(&self) -> Box<dyn Iterator<Item = *mut u8> + '_> {
        match self.size {
            0 => Box::new(std::iter::repeat(self.buf.as_ptr() as *mut u8).take(self.len_elements)),
            _ => Box::new(
                self.buf
                    .chunks(self.size / A)
                    .map(|chunk| chunk as *const [<() as AlignTo<A>>::Aligned] as *mut u8)
                    .take(self.len_elements),
            ),
        }
    }
    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = *mut u8> + '_> {
        match self.size {
            0 => Box::new(
                std::iter::repeat(self.buf.as_mut_ptr() as *mut u8).take(self.len_elements),
            ),
            _ => Box::new(
                self.buf
                    .chunks_mut(self.size / A)
                    .map(|chunk| chunk as *mut [<() as AlignTo<A>>::Aligned] as *mut u8)
                    .take(self.len_elements),
            ),
        }
    }
}
impl dyn ErasedBytesVec + '_ {
    fn unerase_alignement_mut<const A: usize>(&mut self) -> &mut AlignedBytesVec<A>
    where
        (): AlignTo<A>,
    {
        self.erased_as_any_mut().downcast_mut().unwrap()
    }
}

impl<const A: usize> AlignedBytesVec<A>
where
    (): AlignTo<A>,
{
    fn new(size: usize) -> Box<Self> {
        assert!(size == 0 || size % A == 0);
        Box::new(Self {
            inserted_over_space: vec![<_>::default(); size],
            buf: Vec::new(),
            len_elements: 0,
            size,
        })
    }
}

trait AlignTo<const A: usize> {
    type Aligned: Copy + Default;
}

macro_rules! aligned_bytes_type_defs {
    ($($name:ident $num:literal)*) => {
        $(
            #[repr(C, align($num))]
            #[derive(Copy, Clone)]
            struct $name([u8; $num]);
            impl Default for $name {
                fn default() -> Self {
                    Self([0; $num])
                }
            }

            impl AlignTo<$num> for () { type Aligned = $name; }
        )*

        pub fn make_aligned_vec(layout: Layout) -> Box<dyn ErasedBytesVec> {
            match layout.align() {
                $(
                    $num => AlignedBytesVec::<$num>::new(layout.size()),
                )*
                _ => panic!("Invalid alignment, only powers of two up to 2^29 supported"),
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
