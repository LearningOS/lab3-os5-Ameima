// 进程管理相关的系统调用

use crate::loader::get_app_data_by_name;
use crate::mm::{translated_refmut, translated_str};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, TaskStatus,
};
use crate::timer::get_time_us;
use alloc::sync::Arc;
use crate::config::MAX_SYSCALL_NUM;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

// 退出进程，要给出退出码
pub fn sys_exit(exit_code: i32) -> ! {
    debug!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

// 让出处理器
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// 获得pid值
pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

// 复刻进程
pub fn sys_fork() -> isize {
    // 获取当前任务块
    let current_task = current_task().unwrap();
    // 获取新任务任务块
    let new_task = current_task.fork();
    // 获取pid值
    let new_pid = new_task.pid.0;
    // 获取新进程trap上下文
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // 修改子进程的寄存器，子进程fork返回0
    // 这样从内核态返回用户态后，父进程和子进程虽然是两个几乎一模一样的平行空间
    // 但是我们可以在用户态的程序里对fork的返回值做判断而执行不同分支，
    // 这样父进程和子进程就可以利用这一点细小的差别走上不相同的道路了
    trap_cx.x[10] = 0;
    // 压入调度器等待调度
    add_task(new_task);
    new_pid as isize
}

// 使用elf在进程上运行新内容
pub fn sys_exec(path: *const u8) -> isize {
    // 获取地址空间token
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}


// sys_waitpid 是一个立即返回的系统调用，它的返回值语义是：
// 如果当前的进程不存在一个进程 ID 为 pid（pid==-1 或 pid > 0）的子进程，则返回 -1；
// 如果存在一个进程 ID 为 pid 的僵尸子进程，则正常回收并返回子进程的 pid，并更新系统调用的退出码参数为 exit_code 。
// 这里还有一个 -2 的返回值，它的含义是子进程还没退出，通知用户库 user_lib （是实际发出系统调用的地方），
// 这样用户库看到是 -2 后，就进一步调用 sys_yield 系统调用（第46行），让当前父进程进入等待状态。
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    // 获取当前任务控制块
    let task = current_task().unwrap();
    // 寻找子进程

    // ---- 获取任务块可变访问
    let mut inner = task.inner_exclusive_access();
    // 如果要等待的子进程不存在则返回 -1
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- 释放任务块可变访问
    }

    // 等待的子进程存在,看看是不是僵尸进程
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ 获取子进程的访问
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ 释放访问
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // 确认这是对于该子进程控制块的唯一一次强引用
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ 获取子进程的访问
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ 释放访问
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    // 不是僵尸进程就返回-2
    } else {
        -2
    }
    // ---- 释放任务块可变访问
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    // unsafe {
    //     *ts = TimeVal {
    //         sec: us / 1_000_000,
    //         usec: us % 1_000_000,
    //     };
    // }
    0
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    -1
}

// YOUR JOB: 实现sys_set_priority，为任务添加优先级
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    -1
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    -1
}


// YOUR JOB: 实现 sys_spawn 系统调用
// ALERT: 注意在实现 SPAWN 时不需要复制父进程地址空间，SPAWN != FORK + EXEC 
pub fn sys_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let new_task = TaskControlBlock::new(data);
        add_task(new_task);
        new_task.pid.0
    } else {
        -1
    }
}
