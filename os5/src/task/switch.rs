// 切换进程，封装的汇编，使得寄存器保存和恢复由rust编译器抽象机完成

// 汇编嵌入到这里一起编译
core::arch::global_asm!(include_str!("switch.S"));

use super::TaskContext;

extern "C" {
    // 封装的交换函数，主要是把当前的进程的快照存到current_task_cx_ptr里，再把next_task_cx_ptr恢复
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
