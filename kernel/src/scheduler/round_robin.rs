use super::Scheduler;
use super::__schedule_new;
use crate::cpu::{current_stack_top, take_my_cpu};
use crate::sync::interrupt_off;
use crate::task::{idle_task, TaskContext, TaskControlBlock, TaskStatus};
use alloc::{collections::VecDeque, sync::Arc};
use core::arch::asm;
use spin::Mutex;

pub struct RoundRobinScheduler {
    ready_queue: Mutex<VecDeque<Arc<TaskControlBlock>>>,
}

impl RoundRobinScheduler {
    pub fn new() -> Self {
        Self {
            ready_queue: Mutex::new(VecDeque::new()),
        }
    }
}

impl Scheduler for RoundRobinScheduler {
    fn schedule(&self) -> ! {
        interrupt_off();
        // push stack incase overwhelm in schedule -> supervisor_time -> scheduler loop
        let top = current_stack_top();
        unsafe {
            asm!("mv sp, {}", in(reg) top);
        }
        // log::debug!("start schedule");
        if let Some(task) = self.fetch_task() {
            // log::debug!("BT");
            let mut task_inner = task.inner_exclusive_access();
            // log::debug!("AT");
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task PCB manually
            drop(task_inner);
            // add task cx to current cpu
            let mut cpu = take_my_cpu();
            cpu.current = Some(task);
            // release cpu manually
            drop(cpu);
            // schedule new task
            unsafe { __schedule_new(next_task_cx_ptr) }
        } else {
            idle_task();
        }
    }
    fn add_task(&self, task: Arc<TaskControlBlock>) {
        self.ready_queue.lock().push_back(task);
    }
    fn fetch_task(&self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.lock().pop_front()
    }
}
