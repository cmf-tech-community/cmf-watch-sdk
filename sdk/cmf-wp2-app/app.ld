MEMORY
{
  APP (rwx) : ORIGIN = 0, LENGTH = 0x20000
}

ENTRY(_start)

SECTIONS
{
  .wp2info : { KEEP(*(.wp2info)) KEEP(*(.wp2info.features)) } > APP
  .entry : { KEEP(*(.entry)) } > APP
  .text : { *(.text .text.*) } > APP
  .rodata : { *(.rodata .rodata.*) } > APP
  .data : { *(.data .data.*) *(.bss .bss.* COMMON) } > APP
  /DISCARD/ : { *(.ARM.exidx .ARM.exidx.* .ARM.extab.*) }
}
