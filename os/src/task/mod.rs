//! Task management implementation
//!
//! Everything about task management, like starting and switching task is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
use crate::config::*;
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use lazy_static::lazy_static;

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

pub use context::*;
pub use task::*;

// discard
// use self::switch::__switch;

/// the start time of switching.
static mut SWITCH_TIME_START: usize = 0;

/// the total time of switching.
static mut SWITCH_TIME_COUNT: usize = 0;

unsafe fn __switch(
  current_task_cx_ptr: *mut TaskContext,
  next_task_cx_ptr: *const TaskContext,
) {
  SWITCH_TIME_START = get_time_us();
  switch::__switch(current_task_cx_ptr, next_task_cx_ptr);
  SWITCH_TIME_COUNT += get_time_us() - SWITCH_TIME_START;
}

fn get_switch_time_count() -> usize {
  unsafe { SWITCH_TIME_COUNT }
}

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task states transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner`
/// in existing functions on `TaskManager`.
pub struct TaskManager {
  /// total number of tasks.
  num_app: usize,
  /// use inner value to get mutable access.
  inner: UPSafeCell<TaskManagerInner>,
}

impl TaskManager {
  /// Run the first task in task list.
  ///
  /// Generally, the first task in task list is an idle task
  /// (we call it zero process later).
  /// But in ch3, we load apps statically, so the first task is a real app.
  fn run_first_task(&self) -> ! {
    let mut inner = self.inner.exclusive_access();
    let task0 = &mut inner.tasks[0];
    task0.task_status = TaskStatus::Running;
    let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
    inner.refresh_stop_watch();
    drop(inner);

    let mut _unused = TaskContext::zero_init();
    // before this, we should drop local variables that must be dropped manually.
    unsafe {
      __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
    }
    panic!("unreachable in run_first_task!");
  }

  /// Change the status of current `Running` task into `Ready`.
  fn mark_current_suspended(&self) {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    // println!("task {} suspended", current);
    // Statistical kernel time
    inner.tasks[current].kernel_time += inner.refresh_stop_watch();
    inner.tasks[current].task_status = TaskStatus::Ready;
  }

  /// Change the status of current `Running` task into `Exited`.
  fn mark_current_exited(&self) {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    // println!("task {} exited", current);
    // Statistical kernel time and output the time
    inner.tasks[current].kernel_time += inner.refresh_stop_watch();
    println!(
      "[task {} exited. user_time: {} ms, kernel_time: {} ms.]",
      current, inner.tasks[current].user_time, inner.tasks[current].kernel_time
    );
    inner.tasks[current].task_status = TaskStatus::Exited;
  }

  /// Switch current `Running` task to the task we have found,
  /// or there is no `Ready` task and we can exit with all applications completed.
  fn run_next_task(&self) {
    if let Some(next) = self.find_next_task() {
      let mut inner = self.inner.exclusive_access();
      let current = inner.current_task;
      inner.tasks[next].task_status = TaskStatus::Running;
      inner.current_task = next;
      let current_task_cx_ptr =
        &mut inner.tasks[current].task_cx as *mut TaskContext;
      let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
      drop(inner);
      // before this, we should drop local variables that must be dropped manually
      unsafe {
        __switch(current_task_cx_ptr, next_task_cx_ptr);
      }
      // go back to user mode
    } else {
      println!("All applications completed!");

      println!("task switch time: {} us", get_switch_time_count());
    }
  }

  /// Find next task to run and return task id.
  ///
  /// In this case, we only return the first `Ready` task in task list.
  fn find_next_task(&self) -> Option<usize> {
    let inner = self.inner.exclusive_access();
    let current = inner.current_task;
    (current + 1..current + self.num_app + 1)
      .map(|id| id % self.num_app)
      .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
  }

  /// Statistics the kernel time, from now it's user time.
  fn user_time_start(&self) {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    inner.tasks[current].kernel_time += inner.refresh_stop_watch();
  }

  /// Statistics the user time, from now it's kernel time.
  fn user_time_end(&self) {
    let mut inner = self.inner.exclusive_access();
    let current = inner.current_task;
    inner.tasks[current].user_time += inner.refresh_stop_watch();
  }
}

/// Inner of Task Manager
struct TaskManagerInner {
  /// task list
  tasks: [TaskControlBlock; MAX_APP_NUM],
  /// id of current `Running` task
  current_task: usize,
  stop_watch: usize,
}

impl TaskManagerInner {
  fn refresh_stop_watch(&mut self) -> usize {
    let start_time = self.stop_watch;
    self.stop_watch = get_time_us();
    self.stop_watch - start_time
  }
}

lazy_static! {
  /// Global variable: TASK_MANAGER
  pub static ref TASK_MANAGER: TaskManager = {
    let num_app = get_num_app();
    let mut tasks = [TaskControlBlock {
      task_cx: TaskContext::zero_init(),
      task_status: TaskStatus::UnInit,
      kernel_time: 0,
      user_time: 0,
    }; MAX_APP_NUM];

    // for i in 0..num_app {
    //   tasks[i].task_cx = TaskContext::goto_restore(init_app_cx(i));
    //   tasks[i].task_status = TaskStatus::Ready;
    // }
    tasks.iter_mut().enumerate().take(num_app).for_each(|(i,task)| {
      task.task_cx = TaskContext::goto_restore(init_app_cx(i));
      task.task_status = TaskStatus::Ready;
    });

    TaskManager {
      num_app,
      inner: unsafe {
        UPSafeCell::new(TaskManagerInner {
          tasks,
          current_task: 0,
          stop_watch: 0,
        })
      },
    }
  };
}

pub fn suspend_current_and_run_next() {
  mark_current_suspended();
  run_next_task();
}

pub fn exit_current_and_run_exit() {
  mark_current_exited();
  run_next_task();
}

fn mark_current_suspended() {
  TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
  TASK_MANAGER.mark_current_exited();
}

pub fn run_first_task() {
  TASK_MANAGER.run_first_task();
}

fn run_next_task() {
  TASK_MANAGER.run_next_task();
}

pub fn user_time_start() {
  TASK_MANAGER.user_time_start()
}

pub fn user_time_end() {
  TASK_MANAGER.user_time_end()
}
