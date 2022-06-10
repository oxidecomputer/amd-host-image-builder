use amd_efs::ProcessorGeneration;
use amd_flash::Location;

/* Coarse-grained flash locations (in Byte) */

pub(crate) const PSP_BEGINNING: Location = 0x12_0000;
pub(crate) const PSP_END: Location = 0x12_0000 + 0x12_0000;

pub(crate) const BHD_BEGINNING: Location = 0x24_0000;
pub(crate) const BHD_END: Location = 0x24_0000 + 0xA_0000;

pub(crate) const RESET_IMAGE_BEGINNING: Location = 0x30_0000;
pub(crate) const RESET_IMAGE_END: Location = 0xFA_0000;

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
		ProcessorGeneration::Rome | ProcessorGeneration::Milan => {
			0xFA_0000
		}
	}
}
