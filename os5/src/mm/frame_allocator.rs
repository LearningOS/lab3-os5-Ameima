// 实现物理页帧分配器

use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::*;

// 定义物理页帧的资源抽象
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}
// 新分配物理页帧，需要指定页号
impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // 新分配的时候要清零
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}
// 实现Debug特性
impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}
// 实现自动回收特性
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

// 定义页帧分配器特性
trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

// 实现一个栈式页帧分配器
pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

// 栈式页帧分配器初始化，指定可以被分配的页帧范围
impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
        info!("last {} Physical Frames.", self.end - self.current);
    }
    // 剩余可用页帧数
    pub fn remain_num(&self) -> usize {
        self.end - self.current + self.recycled.len()
    }
}
// 为栈式页帧分配器实现页帧分配器特性
impl FrameAllocator for StackFrameAllocator {
    // 新建栈式页帧分配器，可分配范围为0，之后用init指定管理范围
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }
    // 分配页帧，返回页帧号
    fn alloc(&mut self) -> Option<PhysPageNum> {
        // 优先从回收回来的进行弹栈
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        // 没有回收的就从范围中分配新的，如果都没有就返回None
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }
    // 回收页帧，指定页帧号进行回收
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // 如果并没有被分配则panic
        if ppn >= self.current || self.recycled.iter().any(|v| *v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // 压入回收栈
        self.recycled.push(ppn);
    }
}

// 重命名
type FrameAllocatorImpl = StackFrameAllocator;


lazy_static! {
    // 首次访问时初始化页帧分配器，使用内部可变性
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        unsafe { UPSafeCell::new(FrameAllocatorImpl::new()) };
}

// 接口，划定内核以外的区域新建页帧分配器
pub fn init_frame_allocator() {
    // ld导出符号，内核结束位置
    extern "C" {
        fn ekernel();
    }
    // 内核以外的位置都受这个分配器管理
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

// 接口，获得抽象化的物理页帧
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

// 接口，获得剩余可用页帧数
pub fn frame_remain_num() -> usize {
    FRAME_ALLOCATOR.exclusive_access().remain_num()
}

// 给Drop使用，回收页帧
fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

#[allow(unused)]
// 测试页帧分配器是否正常运转
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        info!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        info!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    info!("frame_allocator_test passed!");
}
