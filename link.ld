ENTRY(_start);

SECTIONS
{
  . = 0x0;

  .text : {
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

