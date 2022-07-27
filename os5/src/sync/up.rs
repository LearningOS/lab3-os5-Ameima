// 单线程可安全共享的内部可变RefCell，那些全局分配器之类的都可以用

use core::cell::{RefCell, RefMut};

pub struct UPSafeCell<T> {
    // 包装RefCell
    inner: RefCell<T>,
}

// 实现共享特性
unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    // 新建
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    // 借用一个可变引用，可以像RefCell一样使用
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
