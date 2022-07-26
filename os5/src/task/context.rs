// 任务切换上下文实现

use crate::trap::trap_return;

#[derive(Copy, Clone)]
#[repr(C)]
// 任务切换瞬间的快照，任务上下文
pub struct TaskContext {
    ra: usize, // 函数返回点
    sp: usize, // 栈指针
    s: [usize; 12], // 被调用者保留的寄存器
}

impl TaskContext {
    // 全零的快照，用于unused任务
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
    // 构建一个直接跳转到trap恢复的切换快照
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize, // 返回到trap恢复
            sp: kstack_ptr, // sp指向内核栈顶
            s: [0; 12],
        }
    }
}
