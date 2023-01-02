use amd_efs::ProcessorGeneration;
use amd_flash::Location;

pub(crate) const IMAGE_SIZE: u32 = 32 * 1024 * 1024;

/* Coarse-grained flash locations (in Byte) */

/*

At boot, the flash is read by the PSP.
The data structure that it reads first is Preferred EFH.
From it, the PSP directory and BHD directory is read.
These point to PSP payloads and BHD payloads, respectively.

0x200_0000 +-----------------------------------------+ RESET_IMAGE_END
           |                                         |
           |                                         |
           |                                         |
           |           Reset image (16 MiB)          |
           |                                         |
           |                                         |
           |                                         |
0x100_0000 +-----------------------------------------+ RESET_IMAGE_BEGINNING
           |          Unused (about 384 kiB)         |
           +-----------------------------------------+
           |    Preferred EFH for Rome and Milan     |
 0xFA_0000 +-----------------------------------------+
           |                                         |
           |                                         |
           |             Unused (12 MiB)             |
           |                                         |
           |                                         |
 0x2E_0000 +-----------------------------------------+ BHD_END
           |                                         |
           |                                         |
           |       BHD directory & BHD payloads      |
           |                                         |
           |                                         |
 0x24_0000 +-----------------------------------------+ PSP_END = BHD_BEGINNING
           |                                         |
           |                                         |
           |       PSP directory & PSP payloads      |
           |                                         |
           |                                         |
 0x12_0000 +-----------------------------------------+ PSP_BEGINNING
           |                                         |
           |                                         |
           |           Unused (about 1 MiB)          |
           |                                         |
           |                                         |
           +-----------------------------------------+
           |         Preferred EFH for Naples        |
  0x2_0000 +-----------------------------------------+
           |                                         |
           |             Unused (128 kiB)            |
           |                                         |
       0x0 +-----------------------------------------+

*/

pub const EFH_SIZE: u32 = 0x200;

// Note: This must not be changed.
// It's hardcoded in the PSP bootloader and in amd-efs's "create" function.
/// Note: It's intentionally duplicated so you can get an overview of the
/// memory map by looking at this file. Especially should there be a new
/// generation, you have to adapt this file--and that's on purpose.
#[allow(non_snake_case)]
pub(crate) const fn EFH_BEGINNING(
    processor_generation: ProcessorGeneration,
) -> Location {
    match processor_generation {
        ProcessorGeneration::Naples => 0x2_0000,
        ProcessorGeneration::Rome | ProcessorGeneration::Milan => 0xFA_0000,
    }
}
