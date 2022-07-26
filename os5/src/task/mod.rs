// 任务管理器模块，管理进程

mod context;
mod manager;
mod pid;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;
use lazy_static::*;
use manager::fetch_task;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
pub use manager::add_task;
pub use pid::{pid_alloc, KernelStack, PidHandle};
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
};


// 挂起当前进程，运行下一个进程
pub fn suspend_current_and_run_next() {
    // 获取当前进程的任务控制块
    let task = take_current_task().unwrap();

    // 独占地访问任务控制块
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // 切换到挂起状态
    task_inner.task_status = TaskStatus::Ready;
    // 手动释放
    drop(task_inner);

    // 压回进程调度器
    add_task(task);
    // 切换到空闲上下文
    schedule(task_cx_ptr);
}

// 退出进程，变僵尸，换到下一个进程，需要给出退出码
pub fn exit_current_and_run_next(exit_code: i32) {
    // 直接获取任务控制块的本体，因为一会就要杀掉进程了
    let task = take_current_task().unwrap();
    // **** 访问内部可变部分
    let mut inner = task.inner_exclusive_access();
    // 变僵尸
    inner.task_status = TaskStatus::Zombie;
    // 记录退出码
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ 访问用户初始程序的任务控制块
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        // 移交子进程给用户初始进程
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ 释放用户初始程序的任务控制块

    inner.children.clear();
    // 释放地址空间
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** 释放内部可变部分
    // 释放任务控制块本体
    drop(task);
    // 不需要保存上下文，直接搞个unused就好
    let mut _unused = TaskContext::zero_init();
    // 切换到空闲流
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    // 用户初始程序，创建任务控制块
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        // 通过应用名取出对应的应用的ELF，用于构建任务控制块
        get_app_data_by_name("ch5b_initproc").unwrap()
    ));
}

// 被main函数调用，启动用户初始程序
pub fn add_initproc() {
    // 添加任务
    add_task(INITPROC.clone());
}
