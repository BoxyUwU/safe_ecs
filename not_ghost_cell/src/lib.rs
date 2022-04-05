use core::cell::UnsafeCell;
use std::sync::{Arc, Weak};

pub struct SlowGhostCell<T: ?Sized>(Weak<UnsafeCell<T>>);
pub struct SlowGhostToken<T: ?Sized>(Arc<UnsafeCell<T>>);

unsafe impl<T: Send + Sync + ?Sized> Send for SlowGhostCell<T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for SlowGhostCell<T> {}

unsafe impl<T: Send + Sync + ?Sized> Send for SlowGhostToken<T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for SlowGhostToken<T> {}

impl<T: ?Sized> SlowGhostCell<T> {
    pub fn new<U: ?Sized>(
        data: T,
        map: impl FnOnce(Weak<UnsafeCell<T>>) -> Weak<UnsafeCell<U>>,
    ) -> (SlowGhostCell<U>, SlowGhostToken<T>)
    where
        T: Sized,
    {
        let data = Arc::new(UnsafeCell::new(data));
        let weak = map(Arc::downgrade(&data.clone()));

        assert_eq!(
            Arc::as_ptr(&data) as *const u8,
            Weak::as_ptr(&weak) as *const u8,
        );
        assert_eq!(Arc::strong_count(&data), 1);
        assert_eq!(Arc::weak_count(&data), 1);

        (SlowGhostCell(weak), SlowGhostToken(data))
    }
    pub fn deref<'a, U: ?Sized>(&'a self, token: &'a SlowGhostToken<U>) -> &'a U {
        assert_eq!(
            Arc::as_ptr(&token.0) as *const u8,
            Weak::as_ptr(&self.0) as *const u8,
        );
        assert_eq!(Arc::strong_count(&token.0), 1);
        assert_eq!(Arc::weak_count(&token.0), 1);

        unsafe { &*token.0.get() }
    }
    pub fn deref_mut<'a, U: ?Sized>(&'a self, token: &'a mut SlowGhostToken<U>) -> &'a mut U {
        assert_eq!(
            Arc::as_ptr(&token.0) as *const u8,
            Weak::as_ptr(&self.0) as *const u8,
        );
        assert_eq!(Arc::strong_count(&token.0), 1);
        assert_eq!(Arc::weak_count(&token.0), 1);

        unsafe { &mut *token.0.get() }
    }
    pub fn get_mut<R>(&mut self, func: impl for<'a> FnOnce(Option<&'a mut T>) -> R) -> R {
        assert!(Weak::strong_count(&self.0) <= 1);

        let mut arc = self.0.upgrade();
        let r = func(arc.as_mut().map(|rc| {
            let strong_count = Arc::strong_count(&rc);
            assert!(strong_count == 2 || strong_count == 1);
            assert_eq!(Arc::weak_count(&rc), 1);

            unsafe { &mut *rc.get() }
        }));
        drop(arc);
        r
    }
}
