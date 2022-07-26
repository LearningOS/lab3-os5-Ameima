// 任务控制块的实现

use super::TaskContext;
use super::{pid_alloc, KernelStack, PidHandle};
use crate::config::TRAP_CONTEXT;
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::cell::RefMut;

// 任务控制块分为初始化后就不可变的部分和运行中可变的部分，因为接下来要上Arc了，只能用内部可变
pub struct TaskControlBlock {
    // 初始化后就不变的部分
    pub pid: PidHandle, // 应用的pid句柄，也是一种RAII风格资源抽象
    pub kernel_stack: KernelStack, // 应用的内核栈

    // 运行中发生变化的部分
    inner: UPSafeCell<TaskControlBlockInner>,
}

// 任务控制块的变化部分
pub struct TaskControlBlockInner {
    // Trap上下文的物理页帧号
    pub trap_cx_ppn: PhysPageNum,
    // 应用地址空间中从0x00开始到用户栈结束一共包含多少字节
    pub base_size: usize,
    // 任务切换时挂起的上下文快照
    pub task_cx: TaskContext,
    // 任务状态
    pub task_status: TaskStatus,
    // 任务的地址空间
    pub memory_set: MemorySet,
    // 父进程，使用弱引用
    pub parent: Option<Weak<TaskControlBlock>>,
    // 子进程，使用强引用
    pub children: Vec<Arc<TaskControlBlock>>,
    // 退出码，发生错误或运行结束时设置
    pub exit_code: i32,
}

// 访问可变部分字段的方法
impl TaskControlBlockInner {
    /*
    pub fn get_task_cx_ptr2(&self) -> *const usize {
        &self.task_cx_ptr as *const usize
    }
    */
    // 获取trap上下文
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    // 获取进程的地址空间token
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    // 获取进程状态
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    // 是否是僵尸进程
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

// 任务控制块的方法
impl TaskControlBlock {
    // 获取内部可变的引用
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    // 直接从ELF新建一个进程，获得返回的任务控制块，目前只适用于用户初始程序，其他的靠fork和exec
    pub fn new(elf_data: &[u8]) -> Self {
        // 先用ELF新建进程地址空间
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 获得trap上下文在进程地址空间中的物理地址
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配一个pid，顺便分配内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // 构造任务控制块
        let task_control_block = Self {
            // 不变部分，这两个就是在这里进行初始化就不变了的
            pid: pid_handle, // pid句柄
            kernel_stack, // 内核栈
            // 可变部分
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn, // trap上下文物理页帧号
                    base_size: user_sp, // 从0x00到用户栈顶结束，是整个大小

                    // 调用goto_trap_return方法，构建一个初次进入进程时的任务上下文
                    // 需要提供内核栈顶，这样才能把构造好的任务上下文压到正确的位置（内核栈顶）
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready, //进程状态：挂起
                    memory_set, // 地址空间
                    parent: None, // 直接创建，没有父进程
                    children: Vec::new(), // 子进程为空
                    exit_code: 0, // 退出码初始为0
                })
            },
        };
        // 同时还需要构造trap上下文
        // 获取位置
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context( // 构造trap上下文快照
            entry_point, // 程序入口点，放在trap恢复时执行的位置
            user_sp, // 用户栈指针，创建地址空间时得到的
            KERNEL_SPACE.exclusive_access().token(), // 内核页表，固定写入
            kernel_stack_top, // 内核栈指针，分配Pid时顺便分配的
            trap_handler as usize, // trap处理入口，固定写入
        );
        // 返回任务控制块
        task_control_block
    }
    // 用一个新的elf替代原来进程的内容执行
    pub fn exec(&self, elf_data: &[u8]) {
        // 先用elf创建地址空间
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 获得trap上下文的位置
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        // **** 访问进程控制块的内部可变部分
        let mut inner = self.inner_exclusive_access();
        // 替换地址空间
        inner.memory_set = memory_set;
        // 替换Trap物理页帧号
        inner.trap_cx_ppn = trap_cx_ppn;
        // 构建Trap上下文
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // **** 自动释放内部可变的引用
    }
    // 复刻进程
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        // ---- 独占访问父进程的可变部分
        let mut parent_inner = self.inner_exclusive_access();
        // 复刻父进程的地址空间
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        // 但是trap的物理页帧号还是要自己获取的
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配一个pid，顺便分配内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // 构造任务控制块
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                })
            },
        });
        // 构建父子关系
        parent_inner.children.push(task_control_block.clone());
        // 修改trap上下文中的栈顶指针
        // **** 独占访问子进程可变部分
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // 返回
        task_control_block
        // ---- 释放父进程独占可变部分
        // **** 释放子进程独占可变部分
    }
    // 获取pid值
    pub fn getpid(&self) -> usize {
        self.pid.0
    }
}

#[derive(Copy, Clone, PartialEq)]
// 四种进程状态：未启动、挂起、运行中、僵尸
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Zombie,
}
