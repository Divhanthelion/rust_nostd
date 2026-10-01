/* Linker script for cortexm1 (TI LM3S6965, the chip QEMU's lm3s6965evb emulates).
 * Read-only reference: the checker links with its own pristine copy. */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}

ENTRY(Reset);

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  .vector_table ORIGIN(FLASH) :
  {
    LONG(_stack_start);                    /* word 0: initial stack pointer */
    KEEP(*(.vector_table.reset_vector));   /* word 1: your __RESET_VECTOR */
    KEEP(*(.vector_table.exceptions));     /* words 2-15: your __EXCEPTIONS */
  } > FLASH

  .text : ALIGN(4) { *(.text .text.*); } > FLASH
  .rodata : ALIGN(4) { *(.rodata .rodata.*); . = ALIGN(4); } > FLASH

  .data : ALIGN(4)
  {
    _sdata = .;
    *(.data .data.*);
    . = ALIGN(4);
    _edata = .;
  } > RAM AT > FLASH
  _sidata = LOADADDR(.data);

  .bss (NOLOAD) : ALIGN(4)
  {
    _sbss = .;
    *(.bss .bss.*);
    . = ALIGN(4);
    _ebss = .;
  } > RAM

  /DISCARD/ : { *(.ARM.exidx .ARM.exidx.* .ARM.extab.*); }
}

ASSERT(SIZEOF(.vector_table) == 16 * 4,
  "the vector table must have 16 words: SP, Reset, then 14 exception entries (did you define __RESET_VECTOR and __EXCEPTIONS?)");
