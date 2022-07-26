// 处理器抽象

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

// 处理器资源抽象，处理器主要功能是可以运行进程
pub struct Processor {
    // 当前运行在处理器上的进程
    current: Option<Arc<TaskControlBlock>>,
    // Processor 有一个不同的 idle 控制流，它运行在这个 CPU 核的启动栈上，
    // 功能是尝试从任务管理器中选出一个任务来在当前 CPU 核上执行。
    idle_task_cx: TaskContext, // 空闲控制流
}

// 处理器方法
impl Processor {
    // 初始化处理器
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    // 获取idle 控制流
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    // 获取当前正运行的任务的可写控制块
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    // 获取当前正运行的任务的不可写控制块
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

lazy_static! {
    // 初始化处理器
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

// 开始运行任务，被main函数最后调用，开始进入用户态
pub fn run_tasks() {
    // 死循环，循环等待任务，任务现在是靠的shell发起系统调用传入的
    // 这个循环也被称为空闲上下文，任务让出cpu的时候就会回到这个循环，重新去调度器取新的进程
    loop {
        // 获取处理器修改能力
        let mut processor = PROCESSOR.exclusive_access();
        // 从任务调度器取一个任务，得到任务控制块
        if let Some(task) = fetch_task() {
            // 获取空闲任务上下文
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // 访问任务控制块可变部分
            let mut task_inner = task.inner_exclusive_access();
            // 设置为下一个任务的上下文
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            // 修改为运行中
            task_inner.task_status = TaskStatus::Running;
            // 手动释放，因为后面直接就会去进程里不会回来了
            drop(task_inner);
            // 修改处理器状态
            processor.current = Some(task);
            // 释放处理器
            drop(processor);
            // 从空闲进程切换到下一个进程
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

// 接口，获取当前处理器上正在运行的任务的可写控制块
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

// 接口，获取当前处理器上正在运行的任务的不可写控制块
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

// 获取当前任务的用户地址空间token
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

// 获取当前任务的可写trap上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

// 切换到空闲任务上下文进行新的调度
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    // 获取修改处理器的能力
    let mut processor = PROCESSOR.exclusive_access();
    // 获取空闲进程上下文
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    // 手动释放，因为后面直接就会去进程里不会回来了
    drop(processor);
    // 切换任务
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
