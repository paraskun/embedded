MEMORY
{
  FLASH : ORIGIN = 0x10000000, LENGTH = 4M
  RAM   : ORIGIN = 0x20000000, LENGTH = 520K
}

/* The RP2350 ROM must find the image definition in the first 4 KiB.
 * Keep it immediately after the vectors, rather than as an orphan section
 * after the program. embassy-rp supplies the image definition itself.
 */
SECTIONS {
  .start_block : ALIGN(4) {
    KEEP(*(.start_block));
    KEEP(*(.boot_info));
  } > FLASH
} INSERT AFTER .vector_table;

_stext = ALIGN(ADDR(.start_block) + SIZEOF(.start_block), ALIGNOF(.text));

ASSERT(ADDR(.start_block) + SIZEOF(.start_block) <= ORIGIN(FLASH) + 0x1000,
       "RP2350 image definition must fit in the first 4 KiB of flash");
