# 允许rept使用宏
.altmacro

# 定义保存寄存器和恢复寄存器的宏
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

# 现场保存和恢复，都链接到跳板段里，导出全局符号给rust用
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2
# 保存现场
__alltraps:
    # 在第一句的交换之前，sp存的是用户栈顶，sscratch存的是Trap上下文的固定位置，
    # 现在的切换都是它俩的互换了，用户栈和内核栈都会被存到Trap上下文中，所以恢复的时候从Trap上下文恢复sp而不是sscratch
    # 首先把用户栈顶和Trap上下文寄存器对调，目前还都在用户地址空间里，所以能正常访问
    # Trap上下文是固定位置，所以没必要进行恢复和保存，直接在恢复的时候从rust的常数重新传入即可
    csrrw sp, sscratch, sp

    # 保存剩下的寄存器
    sd x1, 1*8(sp)
    # x2是sp先不存，一会儿把刚才换到sscratch里的塞进去
    sd x3, 3*8(sp)
    # x4不会被应用用到，不保存

    # 保存 x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存sstatus与sepc，这俩寄存器表示当前的特权级与Trap时的处理入口点地址
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # 现在存sp
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # 从上下文的初始化后不变部分读取内核页表token和trap处理入口点
    ld t0, 34*8(sp)
    ld t1, 36*8(sp)

    # 从上下文恢复内核sp，直接覆盖即可，不用管sp中现在存的Trap上下文地址，反正都是固定地址了
    ld sp, 35*8(sp)

    # 把刚才读到的内核页表token写入
    csrw satp, t0
    # 刷新快表
    sfence.vma

    # 跳到 trap_handler
    jr t1

# 恢复现场
__restore:
    # 调用__restore时，a0中放着Trap上下文（以用户地址空间表示），而a1中放着用户地址空间的token
    # 要先换到用户地址空间，才好进行Trap恢复，从a1读取token放进satp，清快表
    csrw satp, a1
    sfence.vma

    # Trap上下文位置放进sscratch中
    csrw sscratch, a0

    # 让sp指向Trap上下文，然后基于sp进行恢复
    mv sp, a0

    # 恢复 sstatus/sepc，这俩寄存器表示当前的特权级与Trap时的处理入口点地址
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1

    # 恢复其余寄存器，除了x0/sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr

    # 把用户栈指针从上下文恢复
    ld sp, 2*8(sp)

    # 回到sepc指示的位置
    sret
