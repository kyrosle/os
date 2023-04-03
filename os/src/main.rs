//! The main module and entrypoint.
//!
//! Various facilities of the kernels are implemented as submodules.
//! The most important ones are:
//!
//! - [`trap`]: Handles all cases of switching from user-space to the kernel.
//! - [`syscall`]: System call handling and implementation.
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality.
//!
//! We then call [`task::run_first_task()`] and for the first time go to user-space.

#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::arch::global_asm;

mod loader;
#[macro_use]
mod console;
mod config;
mod lang_items;
mod sbi;
mod stack_trace;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

// Embed this assembly code.
global_asm!(include_str!("entry.asm"));
// this asm source file is created by build.rs
global_asm!(include_str!("link_app.S"));

/// The rust entry-point of os
#[no_mangle]
pub fn rust_main() -> ! {
  clear_bss();
  println!("[kernel] Kernel program startup");
  trap::init();
  loader::load_apps();
  // setting `sie.stie` interruption won't be masked.
  trap::enable_timer_interrupt();
  // setting a 10 ms time counter.
  timer::set_next_trigger();

  use riscv::register::sstatus;
  unsafe { sstatus::set_sie() }; // open the kernel interrupt
  loop {
    if trap::check_kernel_interrupt() {
      println!("kernel interrupt returned.");
      break;
    }
  }
  unsafe { sstatus::clear_sie() }; // close the kernel interrupt

  task::run_first_task();
  panic!("Unreachable in rust_main!");
}

/// Clear BSS segment
fn clear_bss() {
  extern "C" {
    fn sbss();
    fn ebss();
  }
  (sbss as usize..ebss as usize)
    .for_each(|a| unsafe { (a as *mut u8).write_volatile(0) })
}
