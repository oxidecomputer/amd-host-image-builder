use std::collections::BTreeMap;
use std::path::PathBuf;

use amd_apcb::Apcb;
use serde::Deserialize;

use amd_efs::flash::Location;
use amd_efs::{
    AddressMode, ComboDirectoryEntryFilter, EfhBulldozerSpiMode,
    EfhEspiConfiguration, EfhNaplesSpiMode, EfhRomeSpiMode,
    ProcessorGeneration, PspSoftFuseChain,
};
use amd_efs::{
    BhdDirectoryEntry, BhdDirectoryEntryRegionType, BhdDirectoryEntryType,
    BhdDirectoryRomId,
};
use amd_efs::{
    PspDirectoryEntry, PspDirectoryEntryType, PspDirectoryRomId,
    ValueOrLocation,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Efs {0}")]
    Efs(amd_efs::Error),
    #[error("incompatible executable")]
    IncompatibleExecutable,
    #[error("Io {0}")]
    Io(std::io::Error),
    #[error("image too big")]
    ImageTooBig,
    #[error("psp entry source {0} unknown")]
    PspEntrySourceUnknown(PspDirectoryEntryType),
}

impl From<amd_efs::Error> for Error {
    fn from(err: amd_efs::Error) -> Self {
        Self::Efs(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[derive(Default, Debug)]
pub struct SerdePspDirectoryEntryBlob {
    #[serde(default)]
    pub flash_location: Option<Location>,
    #[serde(default)]
    pub size: Option<u32>, // FIXME u64
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SerdePspDirectoryEntryAttrs {
    #[serde(rename = "type")]
    pub type_: PspDirectoryEntryType,
    /// Function of AMD Family and Model; only useful for types 8, 0x24, 0x25
    #[serde(default)]
    pub sub_program: u8,
    #[serde(default)]
    pub rom_id: PspDirectoryRomId,
    #[serde(default)]
    pub instance: u8, // actually u4
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
        .with_instance(target.attrs.instance)
        .with_sub_program(target.attrs.sub_program)
        .with_rom_id(target.attrs.rom_id)
        .build())
    }
}

#[derive(Clone, serde::Serialize, schemars::JsonSchema)]
#[serde(rename = "SerdePspEntrySourceValue")]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub enum SerdePspEntrySourceValue {
    PspSoftFuseChain(PspSoftFuseChain),
    #[serde(deserialize_with = "deserialize_raw")]
    Unknown(u64),
}

impl<'de> serde::de::Deserialize<'de> for SerdePspEntrySourceValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct PspVisitor;

        impl<'de> serde::de::Visitor<'de> for PspVisitor {
            type Value = SerdePspEntrySourceValue;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter
                    .write_str("a u64 or a SerdePspEntrySourceValue variant")
            }

            fn visit_u64<E>(
                self,
                value: u64,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SerdePspEntrySourceValue::Unknown(value))
            }

            fn visit_i64<E>(
                self,
                value: i64,
            ) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value >= 0 {
                    Ok(SerdePspEntrySourceValue::Unknown(value as u64))
                } else {
                    Err(E::invalid_value(
                        serde::de::Unexpected::Signed(value),
                        &"a positive integer or SerdePspEntrySourceValue variant",
                    ))
                }
            }

            fn visit_map<A>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                if let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "PspSoftFuseChain" => {
                            Ok(SerdePspEntrySourceValue::PspSoftFuseChain(
                                map.next_value::<PspSoftFuseChain>()?,
                            ))
                        }
                        _ => Err(serde::de::Error::custom(
                            "expected SerdePspEntrySourceValue variant",
                        )),
                    }
                } else {
                    Err(serde::de::Error::custom(
                        "expected SerdePspEntrySourceValue variant",
                    ))
                }
            }
        }

        deserializer.deserialize_any(PspVisitor)
    }
}

impl SerdePspEntrySourceValue {
    pub fn from_u64(value: u64, typ: PspDirectoryEntryType) -> Self {
        match typ {
            PspDirectoryEntryType::PspSoftFuseChain => {
                Self::PspSoftFuseChain(PspSoftFuseChain::from(value))
            }
            _ => SerdePspEntrySourceValue::Unknown(value),
        }
    }

    pub fn to_u64(
        &self,
        typ_or_err: std::result::Result<PspDirectoryEntryType, amd_efs::Error>,
    ) -> Result<u64> {
        if let SerdePspEntrySourceValue::Unknown(x) = self {
            Ok(*x)
        } else {
            let typ = typ_or_err.unwrap();
            match typ {
                PspDirectoryEntryType::PspSoftFuseChain => match self {
                    Self::PspSoftFuseChain(x) => Ok(u64::from(*x)),
                    _ => Err(Error::PspEntrySourceUnknown(typ)),
                },
                _ => Err(Error::PspEntrySourceUnknown(typ)),
            }
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspEntrySource")]
#[serde(deny_unknown_fields)]
pub enum SerdePspEntrySource {
    Value(SerdePspEntrySourceValue),
    BlobFile(PathBuf),
    SecondLevelDirectory(SerdePspDirectory),
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdePspEntry {
    pub source: SerdePspEntrySource,
    pub target: SerdePspDirectoryEntry,
}

#[derive(
    Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
impl SerdeBhdDirectoryEntryAttrs {
    pub fn builder() -> Self {
        Self {
            type_: BhdDirectoryEntryType::OemPublicKey,
            region_type: BhdDirectoryEntryRegionType::Normal,
            reset_image: false,
            copy_image: false,
            read_only: false,
            compressed: false,
            instance: 0,
            sub_program: u8::default(),
            rom_id: BhdDirectoryRomId::default(),
        }
    }
    pub fn build(&mut self) -> &mut Self {
        self
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdSource")]
#[serde(deny_unknown_fields)]
pub enum SerdeBhdSource<'a> {
    Implied,
    BlobFile(PathBuf),
    #[serde(bound(deserialize = "Apcb<'a>: Deserialize<'de>"))]
    ApcbJson(amd_apcb::Apcb<'a>),
    SecondLevelDirectory(SerdeBhdDirectory<'a>),
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdEntry")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdEntry<'a> {
    #[serde(bound(deserialize = "SerdeBhdSource<'a>: Deserialize<'de>"))]
    pub source: SerdeBhdSource<'a>, // PathBuf,
    pub target: SerdeBhdDirectoryEntry,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdePspDirectory {
    pub entries: Vec<SerdePspEntry>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "PspComboDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdePspComboDirectory {
    pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdePspDirectory>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
#[serde(deny_unknown_fields)]
pub enum SerdePspDirectoryVariant {
    PspDirectory(SerdePspDirectory),
    PspComboDirectory(SerdePspComboDirectory),
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdDirectory<'a> {
    #[serde(bound(deserialize = "Vec<SerdeBhdEntry<'a>>: Deserialize<'de>"))]
    pub entries: Vec<SerdeBhdEntry<'a>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename = "BhdComboDirectory")]
#[serde(deny_unknown_fields)]
pub struct SerdeBhdComboDirectory<'a> {
    #[serde(bound(
        deserialize = "BTreeMap<ComboDirectoryEntryFilter, SerdeBhdDirectory<'a>>: Deserialize<'de>"
    ))]
    pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdeBhdDirectory<'a>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
struct RawSerdeConfig<'a> {
    pub processor_generation: ProcessorGeneration,
    #[serde(default)]
    pub spi_mode_bulldozer: Option<EfhBulldozerSpiMode>,
    #[serde(default)]
    pub spi_mode_zen_naples: Option<EfhNaplesSpiMode>,
    #[serde(default)]
    pub spi_mode_zen_rome: Option<EfhRomeSpiMode>,
    pub espi0_configuration: Option<EfhEspiConfiguration>,
    pub espi1_configuration: Option<EfhEspiConfiguration>,
    #[serde(alias = "psp_main_directory_location")]
    pub psp_main_directory_flash_location: Option<Location>,
    #[serde(alias = "bhd_main_directory_location")]
    pub bhd_main_directory_flash_location: Option<Location>,
    pub psp: SerdePspDirectoryVariant,
    #[serde(bound(
        deserialize = "SerdeBhdDirectoryVariant<'a>: Deserialize<'de>"
    ))]
    pub bhd: SerdeBhdDirectoryVariant<'a>,
}

// The distinction SerdeConfig vs RawSerdeConfig is so we can validate
// combinations.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawSerdeConfig")]
#[serde(into = "RawSerdeConfig")]
pub struct SerdeConfig<'a> {
    // Note: same fields as above!
    pub processor_generation: ProcessorGeneration,
    pub spi_mode_bulldozer: Option<EfhBulldozerSpiMode>,
    pub spi_mode_zen_naples: Option<EfhNaplesSpiMode>,
    pub spi_mode_zen_rome: Option<EfhRomeSpiMode>,
    pub espi0_configuration: Option<EfhEspiConfiguration>,
    pub espi1_configuration: Option<EfhEspiConfiguration>,
    #[serde(alias = "psp_main_directory_location")]
    pub psp_main_directory_flash_location: Option<Location>,
    #[serde(alias = "bhd_main_directory_location")]
    pub bhd_main_directory_flash_location: Option<Location>,
    pub psp: SerdePspDirectoryVariant,
    pub bhd: SerdeBhdDirectoryVariant<'a>,
}

impl schemars::JsonSchema for SerdeConfig<'_> {
    fn schema_name() -> std::string::String {
        RawSerdeConfig::schema_name()
    }
    fn json_schema(
        generator: &mut schemars::r#gen::SchemaGenerator,
    ) -> schemars::schema::Schema {
        RawSerdeConfig::json_schema(generator)
    }
    fn is_referenceable() -> bool {
        RawSerdeConfig::is_referenceable()
    }
}

impl<'a> From<SerdeConfig<'a>> for RawSerdeConfig<'a> {
    fn from(config: SerdeConfig<'a>) -> Self {
        Self {
            processor_generation: config.processor_generation,
            spi_mode_bulldozer: config.spi_mode_bulldozer,
            spi_mode_zen_naples: config.spi_mode_zen_naples,
            spi_mode_zen_rome: config.spi_mode_zen_rome,
            espi0_configuration: config.espi0_configuration,
            espi1_configuration: config.espi1_configuration,
            psp_main_directory_flash_location: config
                .psp_main_directory_flash_location,
            bhd_main_directory_flash_location: config
                .bhd_main_directory_flash_location,
            psp: config.psp,
            bhd: config.bhd,
        }
    }
}

/// This validates whether the spi mode is compatible with the
/// processor generation (used to validate after deserialization
/// of a json5 config)
impl<'a> core::convert::TryFrom<RawSerdeConfig<'a>> for SerdeConfig<'a> {
    type Error = Error;
    fn try_from(
        raw: RawSerdeConfig<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        match raw.processor_generation {
            ProcessorGeneration::Naples => {
                if raw.spi_mode_bulldozer.is_none()
                    && raw.spi_mode_zen_naples.is_some()
                    && raw.spi_mode_zen_rome.is_none()
                {
                    return Ok(SerdeConfig {
                        processor_generation: raw.processor_generation,
                        spi_mode_bulldozer: raw.spi_mode_bulldozer,
                        spi_mode_zen_naples: raw.spi_mode_zen_naples,
                        spi_mode_zen_rome: raw.spi_mode_zen_rome,
                        espi0_configuration: None,
                        espi1_configuration: None,
                        psp_main_directory_flash_location: raw
                            .psp_main_directory_flash_location,
                        bhd_main_directory_flash_location: raw
                            .bhd_main_directory_flash_location,
                        psp: raw.psp,
                        bhd: raw.bhd,
                    });
                }
            }
            ProcessorGeneration::Rome | ProcessorGeneration::Milan => {
                if raw.spi_mode_bulldozer.is_none()
                    && raw.spi_mode_zen_naples.is_none()
                    && raw.spi_mode_zen_rome.is_some()
                {
                    return Ok(SerdeConfig {
                        processor_generation: raw.processor_generation,
                        spi_mode_bulldozer: raw.spi_mode_bulldozer,
                        spi_mode_zen_naples: raw.spi_mode_zen_naples,
                        spi_mode_zen_rome: raw.spi_mode_zen_rome,
                        espi0_configuration: None,
                        espi1_configuration: None,
                        psp_main_directory_flash_location: raw
                            .psp_main_directory_flash_location,
                        bhd_main_directory_flash_location: raw
                            .bhd_main_directory_flash_location,
                        psp: raw.psp,
                        bhd: raw.bhd,
                    });
                }
            }
            ProcessorGeneration::Genoa | ProcessorGeneration::Turin => {
                // Some Turin images we got from AMD actually set both
                // Bulldozer SPI mode and Turin ESPI configuration.
                // Needless to say, the Bulldozer SPI mode is bullshit.
                if raw.spi_mode_zen_naples.is_none()
                    && raw.spi_mode_zen_rome.is_none()
                    && (raw.espi0_configuration.is_some()
                        || raw.espi1_configuration.is_some())
                {
                    return Ok(SerdeConfig {
                        processor_generation: raw.processor_generation,
                        spi_mode_bulldozer: raw.spi_mode_bulldozer,
                        spi_mode_zen_naples: raw.spi_mode_zen_naples,
                        spi_mode_zen_rome: raw.spi_mode_zen_rome,
                        espi0_configuration: raw.espi0_configuration,
                        espi1_configuration: raw.espi1_configuration,
                        psp_main_directory_flash_location: raw
                            .psp_main_directory_flash_location,
                        bhd_main_directory_flash_location: raw
                            .bhd_main_directory_flash_location,
                        psp: raw.psp,
                        bhd: raw.bhd,
                    });
                }
            }
        }
        Err(Error::Efs(amd_efs::Error::SpiModeMismatch))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProcessorGeneration, RawSerdeConfig, SerdeBhdDirectory,
        SerdeBhdDirectoryVariant, SerdeConfig, SerdePspDirectory,
        SerdePspDirectoryVariant,
    };
    use amd_efs::{
        EfhNaplesSpiMode, EfhRomeSpiMode, SpiFastSpeedNew, SpiNaplesMicronMode,
        SpiReadMode, SpiRomeMicronMode,
    };
    use std::convert::TryFrom;

    #[test]
    #[should_panic(expected = "SpiModeMismatch")]
    fn spi_mode_missing() {
        SerdeConfig::try_from(RawSerdeConfig {
            processor_generation: ProcessorGeneration::Milan,
            psp_main_directory_flash_location: None,
            psp: SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
                entries: vec![],
            }),
            bhd_main_directory_flash_location: None,
            bhd: SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
                entries: vec![],
            }),
            spi_mode_bulldozer: None,
            spi_mode_zen_naples: None,
            spi_mode_zen_rome: None,
            espi0_configuration: None,
            espi1_configuration: None,
        })
        .unwrap();
    }

    #[test]
    fn spi_mode_milan_ok() {
        SerdeConfig::try_from(RawSerdeConfig {
            processor_generation: ProcessorGeneration::Milan,
            psp_main_directory_flash_location: None,
            psp: SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
                entries: [].to_vec(),
            }),
            bhd_main_directory_flash_location: None,
            bhd: SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
                entries: [].to_vec(),
            }),
            spi_mode_bulldozer: None,
            spi_mode_zen_naples: None,
            spi_mode_zen_rome: Some(EfhRomeSpiMode {
                read_mode: SpiReadMode::Normal33_33MHz,
                fast_speed_new: SpiFastSpeedNew::_33_33MHz,
                micron_mode: SpiRomeMicronMode::SupportMicron,
            }),
            espi0_configuration: None,
            espi1_configuration: None,
        })
        .unwrap();
    }

    #[test]
    fn spi_mode_rome_ok() {
        SerdeConfig::try_from(RawSerdeConfig {
            processor_generation: ProcessorGeneration::Rome,
            psp_main_directory_flash_location: None,
            psp: SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
                entries: [].to_vec(),
            }),
            bhd_main_directory_flash_location: None,
            bhd: SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
                entries: [].to_vec(),
            }),
            spi_mode_bulldozer: None,
            spi_mode_zen_naples: None,
            spi_mode_zen_rome: Some(EfhRomeSpiMode {
                read_mode: SpiReadMode::Normal33_33MHz,
                fast_speed_new: SpiFastSpeedNew::_33_33MHz,
                micron_mode: SpiRomeMicronMode::SupportMicron,
            }),
            espi0_configuration: None,
            espi1_configuration: None,
        })
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "SpiModeMismatch")]
    fn spi_mode_naples_not_ok() {
        SerdeConfig::try_from(RawSerdeConfig {
            processor_generation: ProcessorGeneration::Naples,
            psp_main_directory_flash_location: None,
            psp: SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
                entries: [].to_vec(),
            }),
            bhd_main_directory_flash_location: None,
            bhd: SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
                entries: [].to_vec(),
            }),
            spi_mode_bulldozer: None,
            spi_mode_zen_naples: None,
            spi_mode_zen_rome: Some(EfhRomeSpiMode {
                read_mode: SpiReadMode::Normal33_33MHz,
                fast_speed_new: SpiFastSpeedNew::_33_33MHz,
                micron_mode: SpiRomeMicronMode::SupportMicron,
            }),
            espi0_configuration: None,
            espi1_configuration: None,
        })
        .unwrap();
    }

    #[test]
    fn spi_mode_naples_ok() {
        SerdeConfig::try_from(RawSerdeConfig {
            processor_generation: ProcessorGeneration::Naples,
            psp_main_directory_flash_location: None,
            psp: SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
                entries: [].to_vec(),
            }),
            bhd_main_directory_flash_location: None,
            bhd: SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
                entries: [].to_vec(),
            }),
            spi_mode_bulldozer: None,
            spi_mode_zen_naples: Some(EfhNaplesSpiMode {
                read_mode: SpiReadMode::Normal33_33MHz,
                fast_speed_new: SpiFastSpeedNew::_33_33MHz,
                micron_mode: SpiNaplesMicronMode::DummyCycle,
            }),
            spi_mode_zen_rome: None,
            espi0_configuration: None,
            espi1_configuration: None,
        })
        .unwrap();
    }
}
