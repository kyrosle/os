//! Implementation of [`TaskContext`]

use super::TaskStatus;

/// Task Context
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext {
  /// return address ( e.g. __restore ) of __switch ASM function
  ra: usize,
  /// kernel stack pointer of app
  sp: usize,
  /// callee saved registers:  s 0..11
  s: [usize; 12],
}

impl TaskContext {
  /// init task context
  pub fn zero_init() -> Self {
    TaskContext {
      ra: 0,
      sp: 0,
      s: [0; 12],
    }
  }

  pub fn goto_restore(kstack_ptr: usize) -> Self {
    extern "C" {
      fn __restore();
    }
    TaskContext {
      ra: __restore as usize,
      sp: kstack_ptr,
      s: [0; 12],
    }
  }
}
