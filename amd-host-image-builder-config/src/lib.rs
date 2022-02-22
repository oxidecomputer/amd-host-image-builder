use std::convert::TryInto;
use std::path::PathBuf;
use std::collections::BTreeMap;

use amd_efs::AddressMode;
use amd_efs::DirectoryEntry;
use amd_efs::BhdDirectoryEntry;
use amd_efs::BhdDirectoryEntryAttrs;
use amd_efs::EfhBulldozerSpiMode;
use amd_efs::EfhNaplesSpiMode;
use amd_efs::EfhRomeSpiMode;
use amd_efs::ProcessorGeneration;
use amd_efs::PspDirectoryEntry;
use amd_efs::PspDirectoryEntryAttrs;
use amd_efs::ValueOrLocation;
use amd_efs::ComboDirectoryEntryFilter;
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
pub enum SerdePspDirectoryEntryBody {
	Value(u64),
	Blob {
		#[serde(default)]
		flash_location: Option<Location>,
		#[serde(default)]
		size: Option<u32>, // FIXME u64
	}
}

impl Default for SerdePspDirectoryEntryBody {
	fn default() -> Self {
		Self::Blob {
			flash_location: None,
			size: None,
		}
	}
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdePspDirectoryEntry {
	#[serde(flatten)]
	pub attrs: PspDirectoryEntryAttrs,
	#[serde(flatten)]
	#[serde(default)]
	pub body: SerdePspDirectoryEntryBody,
}


#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdePspEntry {
	pub source: PathBuf,
	pub target: SerdePspDirectoryEntry,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum SerdeBhdDirectoryEntryBody {
	Blob {
		#[serde(default)]
		flash_location: Option<Location>,
		#[serde(default)]
		size: Option<u32>, // FIXME u64 ?
		#[serde(default)]
		ram_destination_address: Option<u64>,
	}
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdeBhdDirectoryEntry {
	#[serde(flatten)]
	pub attrs: BhdDirectoryEntryAttrs,
	#[serde(flatten)]
	#[serde(default)]
	pub body: SerdeBhdDirectoryEntryBody,
}

impl Default for SerdeBhdDirectoryEntryBody {
	fn default() -> Self {
		Self::Blob {
			flash_location: None,
			size: None,
			ram_destination_address: None,
		}
	}
}

impl SerdePspDirectoryEntry {
/*
	pub fn load(config: &Self) -> Result<PspDirectoryEntry> {
		match config.body {
			SerdePspDirectoryEntryBody::Value(x) => {
				Ok(PspDirectoryEntry::new_value(&config.attrs, x))
			},
			SerdePspDirectoryEntryBody::Blob { flash_location, size } => {
				let size = size.unwrap();
				Ok(PspDirectoryEntry::new_payload(&config.attrs, size, flash_location.unwrap()).unwrap()) // FIXME .map_err(|_| serde::ser::Error::custom("value unknown"))?
			},
		}
	}
	pub fn save(blob: &PspDirectoryEntry) -> Result<Self> {
		let source = blob.source(AddressMode::DirectoryRelativeOffset); // DirectoryRelativeOffset is the one that can always be overridden
		Ok(SerdePspDirectoryEntry {
			attrs: PspDirectoryEntryAttrs::from(blob.attrs.get()), // .map_err(|_| serde::ser::Error::custom("value unknown"))?.into(),
			body: match source {
				ValueOrLocation::Value(x) => {
					SerdePspDirectoryEntryBody::Value(x)
				},
				ValueOrLocation::Location(x) => {
					let x: u32 = x.try_into().unwrap();
					SerdePspDirectoryEntryBody::Blob {
						flash_location: Some(x.into()), // FIXME
						size: blob.size(),
					}
				},
			},
		})
	}
*/
}

impl SerdeBhdDirectoryEntry {
/*
	pub fn load(config: &Self) -> Result<BhdDirectoryEntry> {
		match config.body {
			SerdeBhdDirectoryEntryBody::Blob { flash_location, size, ram_destination_address } => {
				let flash_location = flash_location.unwrap();
				let size = size.unwrap();
				Ok(BhdDirectoryEntry::new_payload(&config.attrs, size, flash_location, ram_destination_address).unwrap()) // FIXME .map_err(|_| serde::ser::Error::custom("value unknown"))?
			},
		}
	}
	pub fn save(blob: &BhdDirectoryEntry) -> Result<Self> {
		let source = blob.source()?; // FIXME
		Ok(SerdeBhdDirectoryEntry {
			attrs: BhdDirectoryEntryAttrs::from(blob.attrs.get()), // .map_err(|_| serde::ser::Error::custom("value unknown"))?.into(),
			body: match source {
				ValueOrLocation::Value(x) => {
					todo!();
				},
				ValueOrLocation::Location(x) => {
					let x: u32 = x.try_into().unwrap();
					SerdeBhdDirectoryEntryBody::Blob {
						flash_location: Some(x), // FIXME
						size: blob.size(),
						ram_destination_address: blob.destination_location(),
					}
				},
			},
		})
	}
*/
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdeBhdEntry {
	pub source: PathBuf,
	pub target: SerdeBhdDirectoryEntry,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdePspDirectory {
	pub entries: Vec<SerdePspEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdePspComboDirectory {
	pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdePspDirectory>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum SerdePspDirectoryVariant {
	PspDirectory(SerdePspDirectory),
	PspComboDirectory(SerdePspComboDirectory),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdeBhdDirectory {
	pub entries: Vec<SerdeBhdEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdeBhdComboDirectory {
	pub directories: BTreeMap<ComboDirectoryEntryFilter, SerdeBhdDirectory>,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum SerdeBhdDirectoryVariant {
	BhdDirectory(SerdeBhdDirectory),
	BhdComboDirectory(SerdeBhdComboDirectory),
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SerdeConfig {
	pub processor_generation: ProcessorGeneration,
	#[serde(default)]
	pub spi_mode_bulldozer: EfhBulldozerSpiMode,
	#[serde(default)]
	pub spi_mode_zen_naples: EfhNaplesSpiMode,
	#[serde(default)]
	pub spi_mode_zen_rome: EfhRomeSpiMode,
	pub psp: SerdePspDirectoryVariant,
	pub bhd: SerdeBhdDirectoryVariant,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
