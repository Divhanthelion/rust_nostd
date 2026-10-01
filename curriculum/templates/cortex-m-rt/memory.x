/* TI LM3S6965 (QEMU lm3s6965evb). cortex-m-rt's link.x includes this file. */
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   : ORIGIN = 0x20000000, LENGTH = 64K
}
