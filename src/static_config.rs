use amd_efs::ProcessorGeneration;
use amd_efs::flash::Location;

/* Coarse-grained flash locations (in Byte) */

/*

At boot, the flash is read by the PSP.
The data structure that it reads first is Preferred EFH.
From it, the PSP directory and BHD directory is read.
These point to PSP payloads and BHD payloads, respectively.

           +-----------------------------------------+
           |                                         |
           |              BHD payloads               |
           |                                         |
           +-----------------------------------------+
           |    Preferred EFH for Rome and Milan     |
 0xFA_0000 +-----------------------------------------+
           |                                         |
           |                                         |
           |              BHD payloads               |
           |                                         |
           |                                         |
           +-----------------------------------------+
           |                                         |
           |                                         |
           |       BHD directory & BHD payloads      |
           |                                         |
           |                                         |
           +-----------------------------------------+
           |                                         |
           |                                         |
           |       PSP directory & PSP payloads      |
           |                                         |
           |                                         |
           +-----------------------------------------+
           |                                         |
           |                                         |
           |           Unused (about 1 MiB)          |
           |                                         |
           |                                         |
           +-----------------------------------------+
           |    Preferred EFH for Naples and Genoa   |
  0x2_0000 +-----------------------------------------+
           |                                         |
           |             Unused (128 kiB)            |
           |                                         |
       0x0 +-----------------------------------------+

*/

const B: usize = 1;
pub const EFH_SIZE: usize = 0x200 * B;
pub const MAX_PSP_SECOND_LEVEL_DIRECTORY_SIZE: usize = 16384 * B;
pub const MAX_BHD_SECOND_LEVEL_DIRECTORY_SIZE: usize = 16384 * B;
// Needs to be at least 0x1000.
// See also DirectoryAdditionalInfo::with_max_size_checked.
pub const ERASABLE_BLOCK_SIZE: usize = 0x1000;

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
        ProcessorGeneration::Genoa | ProcessorGeneration::Turin => 0x2_0000,
    }
}
