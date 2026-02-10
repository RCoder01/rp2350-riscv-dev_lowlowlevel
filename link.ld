ENTRY(_start);

MEMORY{
        FLASH (rx) : ORIGIN = 0x10000000, LENGTH = 2048K
        RAM (rwx)  : ORIGIN = 0x20000000, LENGTH = 520K
}

_flash_base = ORIGIN(FLASH);
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  . = 0x0;

  .text : {
    KEEP(*(.text._start))
    *(.text._start)
    *(.text._start.*)
    *(.text*)
    *(.rodata*)
  } >FLASH

  .data : {
    *(.data*)
  } >RAM AT>FLASH

  .bss (NOLOAD) : {
    *(.bss*)
    *(COMMON)
  } >RAM

  /DISCARD/ : { *(.eh_frame*) }
}

