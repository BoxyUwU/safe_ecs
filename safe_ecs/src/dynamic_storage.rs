use std::alloc::Layout;

struct AlignedBytesVec<const A: usize>
where
    (): AlignTo<A>,
{
    buf: Vec<<() as AlignTo<A>>::Aligned>,
    size: usize,
}

pub trait ErasedBytesVec {
    fn get_element_ptr(&self, idx: usize) -> *const [u8];
    fn get_element_ptr_mut(&mut self, idx: usize) -> *mut [u8];
    fn realloc(&mut self);
}
impl<const A: usize> ErasedBytesVec for AlignedBytesVec<A>
where
    (): AlignTo<A>,
{
    fn get_element_ptr(&self, idx: usize) -> *const [u8] {
        let byte_start_idx = self.size * idx;
        let byte_end_idx = byte_start_idx + self.size;
        &self.buf[byte_start_idx / A..byte_end_idx / A] as *const _ as *const [u8]
    }

    fn get_element_ptr_mut(&mut self, idx: usize) -> *mut [u8] {
        let byte_start_idx = self.size * idx;
        let byte_end_idx = byte_start_idx + self.size;
        &mut self.buf[byte_start_idx / A..byte_end_idx / A] as *mut _ as *mut [u8]
    }

    fn realloc(&mut self) {
        match self.buf.len() {
            0 => self.buf.resize_with(self.size, Default::default),
            n => self.buf.resize_with(n * 2, Default::default),
        }
    }
}

impl<const A: usize> AlignedBytesVec<A>
where
    (): AlignTo<A>,
{
    fn new(layout: Layout) -> Box<Self> {
        assert!(layout.size() == 0 || layout.size() % layout.align() == 0);
        Box::new(Self {
            buf: Vec::new(),
            size: layout.size(),
        })
    }
}

trait AlignTo<const A: usize> {
    type Aligned: Default;
}

macro_rules! aligned_bytes_type_defs {
    ($($name:ident $num:literal)*) => {
        $(
            #[repr(C, align($num))]
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
                    $num => AlignedBytesVec::<$num>::new(layout),
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
