

# 允许rept使用宏
.altmacro

# 定义保存寄存器和恢复寄存器的宏
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm

# 编译后放在text段，导出__switch供rust调用
    .section .text
    .globl __switch
__switch:
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # )
    # pub struct TaskContext {
    #     ra: usize, // 函数返回点
    #     sp: usize, // 内核栈指针
    #     s: [usize; 12], // 被调用者保留的寄存器
    # }
    # 根据函数调用约定，current_task_cx_ptr在a0，next_task_cx_ptr在a1


    # 保存当前程进程的内核栈顶sp快照到a0的sp位置
    sd sp, 8(a0)
    # 保存当前程序的ra和s寄存器快照到a0的对应位置
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr


    # 从a1恢复下一个进程的ra寄存器和s寄存器快照
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr

    # 从a1恢复下一个进程的内核栈顶指针sp寄存器快照
    ld sp, 8(a1)
    
    ret

