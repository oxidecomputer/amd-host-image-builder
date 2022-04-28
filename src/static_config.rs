
use amd_flash::Location;

/* Coarse-grained flash locations (in Byte) */

// Note: This must not be changed.
// It's hardcoded in the PSP bootloader.
pub(crate) const EFH_BEGINNING: Location = 0x2_0000;

pub(crate) const PSP_BEGINNING: Location = 0xD_0000;
pub(crate) const PSP_END: Location = 0xD_0000 + 0x12_0000;

pub(crate) const BHD_BEGINNING: Location = 0x24_0000;
pub(crate) const BHD_END: Location = 0x24_0000 + 0x3f_0000;
