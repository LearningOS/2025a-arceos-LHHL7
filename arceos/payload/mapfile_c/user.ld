/* 基地址为 0 的链接脚本 */
SECTIONS {
    /* 关键：从 0 开始 */
    . = 0x0;
    
    .text : {
        *(.text .text.*)
    }
    
    . = ALIGN(16);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    
    .bss : {
        *(.bss .bss.*)
        *(COMMON)
        *(.sbss .sbss.*)
    }
    
    /* musl libc 必需符号 */
    __global_pointer$ = . + 0x800;
    _end = .;
    __end = .;
    __bss_start = ADDR(.bss);
    __bss_end = .;
    
    /* 初始化数组符号（简单定义） */
    __init_array_start = .;
    __init_array_end = .;
    __fini_array_start = .;
    __fini_array_end = .;
    __preinit_array_start = .;
    __preinit_array_end = .;
    
    /* 程序头 */
    __ehdr_start = 0x0;
    
    /DISCARD/ : {
        *(.comment)
        *(.note.GNU-stack)
    }
}
