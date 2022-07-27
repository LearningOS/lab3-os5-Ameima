// 进程调度器模块


use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;

// 进程调度器
pub struct TaskManager {
    // 挂起进程的序列，双端队列
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

// YOUR JOB: FIFO->Stride
// 采用Stride调度模型，进程按优先级对应的步长增加长度
// 每次取用长度最短的进程
impl TaskManager {
    // 新建调度器
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    // 将任务压回待调度队列
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    // 从待调度队列弹出最前端的任务
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let mut min_pass: usize = core::usize::MAX;
        let mut min_pass_index: Option<usize> = None;
        for index in 0..self.ready_queue.len() {
            let index_pass = self.ready_queue[index].inner_exclusive_access().task_pass;
            if index_pass <= min_pass {
                min_pass = index_pass;
                min_pass_index = Some(index);
            }
        }
        self.ready_queue.swap_remove_back(min_pass_index.unwrap())
    }
}
// // 采用FIFO调度模型，无优先级，循环排队调度
// impl TaskManager {
//     // 新建调度器
//     pub fn new() -> Self {
//         Self {
//             ready_queue: VecDeque::new(),
//         }
//     }
//     // 将任务压回待调度队列
//     pub fn add(&mut self, task: Arc<TaskControlBlock>) {
//         self.ready_queue.push_back(task);
//     }
//     // 从待调度队列弹出最前端的任务
//     pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
//         self.ready_queue.pop_front()
//     }
// }

lazy_static! {
    // 初始化调度器
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

// 接口，任务压回调度器
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

// 接口，从调度器取一个任务
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
