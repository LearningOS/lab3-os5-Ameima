// SV39分页内核管理模块

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

// 从子模块导出出来，mod.rs作为可见性屏障
pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, translated_refmut, translated_str, PageTableEntry};
use page_table::{PTEFlags, PageTable};

// 初始化内存管理模块
pub fn init() {
    // 初始化内核堆
    heap_allocator::init_heap();
    // 初始化物理页帧分配器
    frame_allocator::init_frame_allocator();
    // 创建内核地址空间，内核页表放入寄存器，启用分页模式
    KERNEL_SPACE.exclusive_access().activate();
}
