ENTRY(_start);

SECTIONS
{
  . = 0x0;

  .text : {
    KEEP(*(.text._start))
    *(.text._start)
    *(.text._start.*)
    *(.text*)
    *(.rodata*)
  }

  .data : {
    *(.data*)
  }

  .bss (NOLOAD) : {
    *(.bss*)
    *(COMMON)
  }

  /DISCARD/ : { *(.eh_frame*) }
}

