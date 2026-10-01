/* Memory layout of the TI LM3S6965 (the MCU QEMU's lm3s6965evb board emulates).
   Adapt ORIGIN/LENGTH for your chip: e.g. an STM32F4 has FLASH at 0x08000000. */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}

/* The CPU doesn't care about ELF entry points, but debuggers and QEMU do. */
ENTRY(Reset);

/* Full-descending stack starting at the top of RAM. */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  /* The vector table must sit at the start of flash (address 0 on boot). */
  .vector_table ORIGIN(FLASH) :
  {
    LONG(_stack_start);                    /* word 0: initial SP */
    KEEP(*(.vector_table.reset_vector));   /* word 1: Reset handler */
    KEEP(*(.vector_table.exceptions));     /* words 2..15: exceptions */
  } > FLASH

  .text : ALIGN(4)
  {
    *(.text .text.*);
  } > FLASH

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > FLASH

  /* Initialised data: lives in RAM, but its initial values are stored in
     flash right after .rodata. Reset copies them over. */
  .data : ALIGN(4)
  {
    _sdata = .;
    *(.data .data.*);
    . = ALIGN(4);
    _edata = .;
  } > RAM AT > FLASH
  _sidata = LOADADDR(.data);

  /* Zero-initialised data: Reset fills it with zeros. */
  .bss (NOLOAD) : ALIGN(4)
  {
    _sbss = .;
    *(.bss .bss.*);
    . = ALIGN(4);
    _ebss = .;
  } > RAM

  /* We abort on panic, so unwind tables are dead weight. */
  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.* .ARM.extab.*);
  }
}

ASSERT(SIZEOF(.vector_table) == 16 * 4, "the Cortex-M3 vector table needs 16 entries here");
ASSERT(_stack_start - _ebss >= 4K, "less than 4 KiB left for the stack");
