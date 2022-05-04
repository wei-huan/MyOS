use super::Scheduler;
use super::__schedule;
use crate::cpu::take_my_cpu;
use crate::task::{TaskContext, TaskControlBlock, TaskStatus};
use alloc::{collections::VecDeque, sync::Arc};
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
    fn schedule(&self) {
        // log::trace!("Start Schedule");
        loop {
            if let Some(task) = self.fetch_task() {
                log::debug!("have task");
                let mut task_inner = task.acquire_inner_lock();
                let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
                task_inner.task_status = TaskStatus::Running;
                // release coming task PCB manually
                drop(task_inner);
                // add task cx to current cpu
                let mut cpu = take_my_cpu();
                let idle_task_cx_ptr = cpu.get_idle_task_cx_ptr();
                cpu.current = Some(task);
                // release cpu manually
                drop(cpu);
                // schedule new task
                unsafe { __schedule(idle_task_cx_ptr, next_task_cx_ptr) }
            }
        }
    }
    fn add_task(&self, task: Arc<TaskControlBlock>) {
        self.ready_queue.lock().push_back(task);
    }
    fn fetch_task(&self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.lock().pop_front()
    }
}
