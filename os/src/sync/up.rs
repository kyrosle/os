//! UniProcessor interior mutability primitives.

use core::cell::{RefCell, RefMut};

/// Wrap a static data structure inside it so that we are
/// able to access it without any `unsafe`.
///
/// We should only use it in uniProcessor.
///
/// In order to get mutable references of inner data, call `exclusive_access`.
///
/// Allows us to safely use mutable global variables on single core .
pub struct UPSafeCell<T> {
  /// inner data
  inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
  /// User is responsible to guarantee that inner struct is only used in UniProcessor.
  pub unsafe fn new(value: T) -> Self {
    Self {
      inner: RefCell::new(value),
    }
  }
  /// Panic if the data has been borrowed.
  pub fn exclusive_access(&self) -> RefMut<'_, T> {
    self.inner.borrow_mut()
  }
}