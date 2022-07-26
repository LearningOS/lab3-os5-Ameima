// pid实现。进程的唯一标识符，同时唯一标识进程的内核栈

use crate::config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE};
use crate::mm::{MapPermission, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use lazy_static::*;

// pid作为一种资源，也使用分配器和自动回收
struct PidAllocator {
    current: usize, // 新分配pid，自增即可
    recycled: Vec<usize>, // 回收的pid
}

// pid分配器的方法
impl PidAllocator {
    // 新建分配器
    pub fn new() -> Self {
        PidAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    // 分配pid句柄
    pub fn alloc(&mut self) -> PidHandle {
        // 用二手的
        if let Some(pid) = self.recycled.pop() {
            PidHandle(pid)
        // 用全新的
        } else {
            self.current += 1;
            PidHandle(self.current - 1)
        }
    }
    // 回收pid句柄
    pub fn dealloc(&mut self, pid: usize) {
        // 检查是否是未分配出去的
        assert!(pid < self.current);
        assert!(
            !self.recycled.iter().any(|ppid| *ppid == pid),
            "pid {} has been deallocated!",
            pid
        );
        self.recycled.push(pid);
    }
}

lazy_static! {
    // 定义一个pid分配器
    static ref PID_ALLOCATOR: UPSafeCell<PidAllocator> =
        unsafe { UPSafeCell::new(PidAllocator::new()) };
}

// pid的资源抽象，也就是pid句柄
pub struct PidHandle(pub usize);

// 自动回收
impl Drop for PidHandle {
    fn drop(&mut self) {
        //println!("drop pid {}", self.0);
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

// 接口，分配pid句柄
pub fn pid_alloc() -> PidHandle {
    PID_ALLOCATOR.exclusive_access().alloc()
}

// 通过pid查询进程的内核栈栈顶和栈底应该分配在哪，内核栈是根据进程id进行从上到下按序按需线性分配的
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

// 内核栈同样也可以视为资源，使用RAII风格对资源进行借用
pub struct KernelStack {
    pid: usize,
}

// 内核栈的方法
impl KernelStack {
    // 给进程句柄新分配一个内核栈
    pub fn new(pid_handle: &PidHandle) -> Self {
        // 获取pid数值
        let pid = pid_handle.0;
        // 查询这个pid对应的内核栈应该分配在哪
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(pid);
        // 内核空间中插入一片用页帧分配器管理的地址用作这个进程的内核栈
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        // 封装资源抽象并返回
        KernelStack { pid: pid_handle.0 }
    }
    #[allow(unused)]
    // 内核栈把类型T的变量压栈，返回一个指向栈顶的该类型的指针
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }
    // 获取栈顶指针
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.pid);
        kernel_stack_top
    }
}

// 自动回收内核栈资源
impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.pid);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}
