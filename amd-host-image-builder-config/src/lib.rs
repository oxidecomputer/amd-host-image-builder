use std::collections::BTreeMap;
use std::path::PathBuf;

use amd_apcb::Apcb;
use serde::Deserialize;

use amd_efs::{
    AddressMode, ComboDirectoryEntryFilter, EfhBulldozerSpiMode,
    EfhNaplesSpiMode, EfhRomeSpiMode, ProcessorGeneration,
};
use amd_efs::{
    BhdDirectoryEntry, BhdDirectoryEntryRegionType, BhdDirectoryEntryType,
    BhdDirectoryRomId,
};
use amd_efs::{
    PspDirectoryEntry, PspDirectoryEntryType, PspDirectoryRomId,
    ValueOrLocation,
};
use amd_flash::Location;

#[derive(Debug)]
pub enum Error {
    Efs(amd_efs::Error),
    IncompatibleExecutable,
    Io(std::io::Error),
    ImageTooBig,
}

impl From<amd_efs::Error> for Error {
    fn from(err: amd_efs::Error) -> Self {
        Self::Efs(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[derive(Default, Debug)]
pub struct SerdePspDirectoryEntryBlob {
    #[serde(default)]
    pub flash_location: Option<Location>,
    #[serde(default)]
    pub size: Option<u32>, // FIXME u64
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SerdePspDirectoryEntryAttrs {
    #[serde(rename = "type")]
    pub type_: PspDirectoryEntryType,
    /// Function of AMD Family and Model; only useful for types 8, 0x24, 0x25
    #[serde(default)]
    pub sub_program: u8,
    #[serde(default)]
    pub rom_id: PspDirectoryRomId,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspDirectoryEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdePspDirectoryEntry {
    #[serde(flatten)]
    pub attrs: SerdePspDirectoryEntryAttrs,

    #[serde(flatten)]
    pub blob: Option<SerdePspDirectoryEntryBlob>,
}

pub trait TryFromSerdeDirectoryEntryWithContext<S>: Sized {
    fn try_from_with_context(
        directory_address_mode: AddressMode,
        source: &S,
    ) -> Result<Self>;
}

// TODO: Generate.
impl TryFromSerdeDirectoryEntryWithContext<SerdePspDirectoryEntry>
    for PspDirectoryEntry
{
    fn try_from_with_context(
        directory_address_mode: AddressMode,
        target: &SerdePspDirectoryEntry,
    ) -> Result<Self> {
        let blob = target.blob.as_ref();
        Ok(Self::new_payload(
            directory_address_mode,
            target.attrs.type_,
            blob.and_then(|y| y.size),
            blob.and_then(|x| {
                x.flash_location.map(ValueOrLocation::EfsRelativeOffset)
            }),
        )?
        .with_sub_program(target.attrs.sub_program)
        .with_rom_id(target.attrs.rom_id)
        .build())
    }
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspEntrySource")]
#[serde(deny_unknown_fields)]
pub enum SerdePspEntrySource {
    Value(u64),
    BlobFile(PathBuf),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdePspEntry {
    pub source: SerdePspEntrySource,
    pub target: SerdePspDirectoryEntry,
}

#[derive(Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdDirectoryEntryBlob")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdDirectoryEntryBlob {
    #[serde(default)]
    pub flash_location: Option<Location>,
    #[serde(default)]
    pub size: Option<u32>, // FIXME u64 ?
    #[serde(default)]
    pub ram_destination_address: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdDirectoryEntryAttrs {
    #[serde(rename = "type")]
    pub type_: BhdDirectoryEntryType,
    #[serde(default)]
    pub region_type: BhdDirectoryEntryRegionType,
    #[serde(default)]
    pub reset_image: bool,
    #[serde(default)]
    pub copy_image: bool,
    #[serde(default)] // for x86: the only choice
    pub read_only: bool,
    #[serde(default)]
    pub compressed: bool,
    #[serde(default)]
    pub instance: u8,
    /// Function of AMD Family and Model; only useful for types PMU firmware and APCB binaries
    #[serde(default)]
    pub sub_program: u8,
    #[serde(default)]
    pub rom_id: BhdDirectoryRomId,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdDirectoryEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdDirectoryEntry {
    #[serde(flatten)]
    pub attrs: SerdeBhdDirectoryEntryAttrs,

    #[serde(flatten)]
    pub blob: Option<SerdeBhdDirectoryEntryBlob>,
}

// TODO: Generate.
impl TryFromSerdeDirectoryEntryWithContext<SerdeBhdDirectoryEntry>
    for BhdDirectoryEntry
{
    fn try_from_with_context(
        directory_address_mode: AddressMode,
        target: &SerdeBhdDirectoryEntry,
    ) -> Result<Self> {
        let blob = target.blob.as_ref();
        Ok(Self::new_payload(
            directory_address_mode,
            target.attrs.type_,
            blob.and_then(|y| y.size),
            blob.and_then(|x| {
                x.flash_location.map(ValueOrLocation::EfsRelativeOffset)
            }),
            blob.and_then(|y| y.ram_destination_address),
        )?
        .with_region_type(target.attrs.region_type)
        .with_reset_image(target.attrs.reset_image)
        .with_copy_image(target.attrs.copy_image)
        .with_read_only(target.attrs.read_only)
        .with_compressed(target.attrs.compressed)
        .with_instance(target.attrs.instance)
        .with_sub_program(target.attrs.sub_program)
        .with_rom_id(target.attrs.rom_id)
        .build())
    }
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdSource")]
#[serde(deny_unknown_fields)]
pub enum SerdeBhdSource<'a> {
    Implied,
    BlobFile(PathBuf),
    #[serde(bound(deserialize = "Apcb<'a>: Deserialize<'de>"))]
    ApcbJson(amd_apcb::Apcb<'a>),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdEntry<'a> {
    #[serde(bound(deserialize = "SerdeBhdSource<'a>: Deserialize<'de>"))]
    pub source: SerdeBhdSource<'a>, // PathBuf,
    pub target: SerdeBhdDirectoryEntry,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdePspDirectory {
    pub entries: Vec<SerdePspEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspComboDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdePspComboDirectory {
    pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdePspDirectory>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum SerdePspDirectoryVariant {
    PspDirectory(SerdePspDirectory),
    PspComboDirectory(SerdePspComboDirectory),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdDirectory<'a> {
    #[serde(bound(deserialize = "Vec<SerdeBhdEntry<'a>>: Deserialize<'de>"))]
    pub entries: Vec<SerdeBhdEntry<'a>>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdComboDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdComboDirectory<'a> {
    #[serde(bound(
        deserialize = "BTreeMap<ComboDirectoryEntryFilter, SerdeBhdDirectory<'a>>: Deserialize<'de>"
    ))]
    pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdeBhdDirectory<'a>>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdDirectoryVariant")]
#[serde(deny_unknown_fields)]
pub enum SerdeBhdDirectoryVariant<'a> {
    #[serde(bound(deserialize = "SerdeBhdDirectory<'a>: Deserialize<'de>"))]
    BhdDirectory(SerdeBhdDirectory<'a>),
    #[serde(bound(
        deserialize = "SerdeBhdComboDirectory<'a>: Deserialize<'de>"
    ))]
    BhdComboDirectory(SerdeBhdComboDirectory<'a>),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "Config")]
#[serde(deny_unknown_fields)]
pub struct SerdeConfig<'a> {
    pub processor_generation: ProcessorGeneration,
    #[serde(default)]
    pub spi_mode_bulldozer: EfhBulldozerSpiMode,
    #[serde(default)]
    pub spi_mode_zen_naples: EfhNaplesSpiMode,
    #[serde(default)]
    pub spi_mode_zen_rome: EfhRomeSpiMode,
    pub psp: SerdePspDirectoryVariant,
    #[serde(bound(
        deserialize = "SerdeBhdDirectoryVariant<'a>: Deserialize<'de>"
    ))]
    pub bhd: SerdeBhdDirectoryVariant<'a>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
