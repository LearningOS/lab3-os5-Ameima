/*输出平台*/
OUTPUT_ARCH(riscv)
/*入口点*/
ENTRY(_start)
/*内核基址*/
BASE_ADDRESS = 0x80200000;

/*段设定*/
SECTIONS
{
    /*从基址开始*/
    . = BASE_ADDRESS;
    /*内核开始点符号*/
    skernel = .;

    /*代码段开始符号*/
    stext = .;
    .text : {
        /*入口点放在最前面*/
        *(.text.entry)
        . = ALIGN(4K);
        /*然后是跳板段*/
        strampoline = .;
        *(.text.trampoline);
        /*其余的代码段*/
        . = ALIGN(4K);
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    /*只读全局常量存储区*/
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    /*全局变量与用户态程序存储区*/
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    /*内核堆栈*/
    sbss_with_stack = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /*不链接调试信息*/
    /DISCARD/ : {
        *(.eh_frame)
    }
}