use amd_efs::{
	AddressMode, BhdDirectory, BhdDirectoryEntry, BhdDirectoryEntryType,
	DirectoryEntry, Efs, PspDirectory, PspDirectoryEntry, ValueOrLocation,
	PspDirectoryEntryType,
};
use amd_host_image_builder_config::{
	Error, Result, SerdeBhdDirectoryVariant, SerdeBhdSource,
	SerdePspDirectoryVariant, SerdePspEntrySource,
	TryFromSerdeDirectoryEntryWithContext,
};
use core::cell::RefCell;
use core::convert::TryFrom;
use core::convert::TryInto;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

mod static_config;

use amd_flash::{ErasableLocation, FlashRead, FlashWrite, Location};
use amd_host_image_builder_config::SerdeConfig;

#[test]
fn test_bitfield_serde() {
	let config = r#"{
	"max_size": 2,
	"base_address": 3,
	"address_mode": "PhysicalAddress"
}"#;
	use amd_efs::DirectoryAdditionalInfo;
	let result: DirectoryAdditionalInfo = json5::from_str(config).unwrap();
	assert_eq!(result.address_mode(), AddressMode::PhysicalAddress);
}

mod hole;
use hole::Hole;

struct FlashImage {
	file: RefCell<File>,
	filename: PathBuf,
}

impl<const ERASABLE_BLOCK_SIZE: usize> FlashRead<ERASABLE_BLOCK_SIZE>
	for FlashImage
{
	fn read_exact(
		&self,
		location: Location,
		buffer: &mut [u8],
	) -> amd_flash::Result<usize> {
		let mut file = self.file.borrow_mut();
		match file.seek(SeekFrom::Start(location.into())) {
			Ok(_) => {}
			Err(e) => {
				eprintln!("Error seeking in flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
		match file.read_exact(buffer) {
			Ok(()) => Ok(buffer.len()),
			Err(e) => {
				eprintln!("Error reading from flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
	}
	fn read_erasable_block(
		&self,
		location: ErasableLocation<ERASABLE_BLOCK_SIZE>,
		buffer: &mut [u8; ERASABLE_BLOCK_SIZE],
	) -> amd_flash::Result<()> {
		let location = Location::from(location);
		let mut file = self.file.borrow_mut();
		match file.seek(SeekFrom::Start(location.into())) {
			Ok(_) => {}
			Err(e) => {
				eprintln!("Error seeking in flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
		match file.read(buffer) {
			Ok(size) => {
				assert!(size == ERASABLE_BLOCK_SIZE);
				Ok(())
			}
			Err(e) => {
				eprintln!("Error reading from flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
	}
}

impl<const ERASABLE_BLOCK_SIZE: usize> FlashWrite<ERASABLE_BLOCK_SIZE>
	for FlashImage
{
	fn erase_block(
		&self,
		location: ErasableLocation<ERASABLE_BLOCK_SIZE>,
	) -> amd_flash::Result<()> {
		let location = Location::from(location);
		let mut file = self.file.borrow_mut();
		match file.seek(SeekFrom::Start(location.into())) {
			Ok(_) => {}
			Err(e) => {
				eprintln!("Error seeking in flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
		let buffer = [0xFFu8; ERASABLE_BLOCK_SIZE];
		match file.write(&buffer[..]) {
			Ok(size) => {
				assert!(size == ERASABLE_BLOCK_SIZE);
				Ok(())
			}
			Err(e) => {
				eprintln!("Error writing to flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
	}
	fn erase_and_write_block(
		&self,
		location: ErasableLocation<ERASABLE_BLOCK_SIZE>,
		buffer: &[u8; ERASABLE_BLOCK_SIZE],
	) -> amd_flash::Result<()> {
		let location = Location::from(location);
		let mut file = self.file.borrow_mut();
		match file.seek(SeekFrom::Start(location.into())) {
			Ok(_) => {}
			Err(e) => {
				eprintln!("Error seeking in flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
		match file.write(&(*buffer)[..]) {
			Ok(size) => {
				assert!(size == ERASABLE_BLOCK_SIZE);
				Ok(())
			}
			Err(e) => {
				eprintln!("Error writing to flash image {:?}: {:?}", self.filename, e);
				return Err(amd_flash::Error::Io);
			}
		}
	}
}

impl FlashImage {
	fn new(file: File, filename: &Path) -> Self {
		Self {
			file: RefCell::new(file),
			filename: filename.to_path_buf(),
		}
	}
}

const ERASABLE_BLOCK_SIZE: usize = 0x1000;
type AlignedLocation = ErasableLocation<ERASABLE_BLOCK_SIZE>;

/// Open SOURCE_FILENAME and checks its size.
/// If TARGET_SIZE is given, make sure the file is at most as big as that.
/// If file is too big, error out.
/// Otherwise, return the size to use for the entry payload.
fn size_file(
	source_filename: &Path,
	target_size: Option<u32>,
) -> amd_efs::Result<(File, u32)> {
	let file = match File::open(source_filename) {
		Ok(f) => f,
		Err(e) => {
			panic!(
				"Could not open file {:?}: {}",
				source_filename, e
			);
		}
	};
	let filesize: usize = file
		.metadata()
		.map_err(|_| amd_efs::Error::Io(amd_flash::Error::Io))?
		.len()
		.try_into()
		.map_err(|_| amd_efs::Error::Io(amd_flash::Error::Io))?;
	match target_size {
		Some(x) => {
			if filesize > x as usize {
				panic!("Configuration specifies slot size {} but contents {:?} have size {}. The contents do not fit.", x, source_filename, filesize);
			} else {
				Ok((file, x))
			}
		}
		None => Ok((file, filesize.try_into().unwrap())),
	}
}

/// Reads the file named SOURCE_FILENAME, finds the version field in there (if any) and returns
/// its value.
/// In case of error (file can't be read, version field not found, ...),
/// returns None.
fn psp_file_version(source_filename: &Path) -> Option<u32> {
	// Note: This does not work on Rome and neither do we know a useful
	// ABL version there anyway. But the magic also doesn't match, so
	// this will be fine.
	let (file, _size) = size_file(source_filename, None).ok()?;
	let mut source = BufReader::new(file);
	let mut header: [u8; 0x100] = [0; 0x100];
	if let Ok(_) = source.read_exact(&mut header) {
		let magic = &header[0x10..0x14];
		if magic == *b"$PS1" {
			let version_raw = <[u8; 4]>::try_from(&header[0x60..0x64]).ok()?;
			let version = u32::from_le_bytes(version_raw);
			Some(version)
		} else {
			None
		}
	} else {
		None
	}
}

/// Add entry from file SOURCE_FILENAME.
/// Errors out if file SOURCE_FILENAME is bigger than ENTRY.size.
/// If successful, updates ENTRY.size to the actual size.
fn psp_entry_add_from_file_with_custom_size(
	directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	entry: &mut PspDirectoryEntry,
	target_size: Option<u32>,
	source_filename: &Path,
) -> amd_efs::Result<()> {
	//eprintln!("FILE {:?}", source_filename);
	let (file, size) = size_file(source_filename, target_size)?;
	entry.set_size(Some(size));
	let mut source = BufReader::new(file);

	directory.add_from_reader_with_custom_size(
		payload_position,
		entry,
		&mut source,
	)
}

fn psp_entry_add_from_file(
	directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	entry: &mut PspDirectoryEntry,
	source_filename: PathBuf,
) -> amd_efs::Result<()> {
	let source_filename = source_filename.as_path();
	psp_entry_add_from_file_with_custom_size(
		directory,
		payload_position,
		entry,
		None,
		&source_filename,
	)
}

/// Add entry from file SOURCE_FILENAME.  Size the payload of entry to TARGET_SIZE.
/// Errors out if file SOURCE_FILENAME is bigger than TARGET_SIZE.
fn bhd_entry_add_from_file_with_custom_size(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	entry: &mut BhdDirectoryEntry,
	target_size: Option<u32>,
	source_filename: &Path,
) -> amd_efs::Result<()> {
	let (file, size) = size_file(source_filename, target_size.into())?;
	entry.set_size(Some(size));
	let mut reader = BufReader::new(file);

	directory.add_from_reader_with_custom_size(
		payload_position,
		entry,
		&mut reader,
	)
}

fn bhd_entry_add_from_file(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	entry: &mut BhdDirectoryEntry,
	source_filename: PathBuf,
) -> amd_efs::Result<()> {
	let source_filename = source_filename.as_path();
	bhd_entry_add_from_file_with_custom_size(
		directory,
		payload_position,
		entry,
		None,
		&source_filename,
	)
}

fn elf_symbol(
	binary: &goblin::elf::Elf,
	key: &str,
) -> Option<goblin::elf::Sym> {
	for sym in &binary.syms {
		let ix = sym.st_name;
		if ix != 0 {
			if &binary.strtab[sym.st_name] == key {
				return Some(sym);
			}
		}
	}
	None
}

fn bhd_directory_add_reset_image(
	bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	reset_image_filename: &Path,
) -> Result<()> {
	let buffer =
		fs::read(reset_image_filename).map_err(|x| Error::Io(x))?;
	let mut destination_origin: Option<u64> = None;
	let mut iov = Box::new(std::io::empty()) as Box<dyn Read>;
	let sz;

	match goblin::Object::parse(&buffer)
		.map_err(|_| Error::IncompatibleExecutable)?
	{
		goblin::Object::Elf(binary) => {
			let mut last_vaddr = 0u64;
			let mut holesz = 0usize;
			let mut totalsz = 0usize;
			if binary.header.e_type != goblin::elf::header::ET_EXEC ||
				binary.header.e_machine !=
					goblin::elf::header::EM_X86_64 || binary
				.header
				.e_version <
				goblin::elf::header::EV_CURRENT.into()
			{
				return Err(Error::IncompatibleExecutable);
			}
			for header in &binary.program_headers {
				if header.p_type ==
					goblin::elf::program_header::PT_LOAD
				{
					//eprintln!("PROG {:x?}", header);
					if header.p_memsz == 0 {
						continue;
					}
					if destination_origin == None {
						// Note: File is sorted by p_vaddr.
						destination_origin =
							Some(header.p_vaddr);
						last_vaddr = header.p_vaddr;
					}
					if header.p_vaddr < last_vaddr {
						// According to ELF standard, this should not happen
						return Err(Error::IncompatibleExecutable);
					}
					if header.p_filesz > header.p_memsz {
						// According to ELF standard, this should not happen
						return Err(Error::IncompatibleExecutable);
					}
					if header.p_paddr != header.p_vaddr {
						return Err(Error::IncompatibleExecutable);
					}
					if header.p_filesz > 0 {
						if header.p_vaddr > last_vaddr {
							holesz += (header
								.p_vaddr -
								last_vaddr)
								as usize;
						}
						if holesz > 0 {
							//eprintln!("hole: {:x}", holesz);
							iov = Box::new(iov.chain(Hole::new(holesz))) as Box<dyn Read>;
							totalsz += holesz;
							holesz = 0;
						}
						let chunk = &buffer[header
							.p_offset
							as usize ..
							(header.p_offset +
								header.p_filesz)
								as usize];
						//eprintln!("chunk: {:x} @ {:x}", header.p_filesz, header.p_offset);
						iov = Box::new(iov.chain(chunk))
							as Box<dyn Read>;
						totalsz += header.p_filesz
							as usize;
						if header.p_memsz >
							header.p_filesz
						{
							holesz += (header
								.p_memsz -
								header.p_filesz)
								as usize;
						}
						last_vaddr = header.p_vaddr +
							header.p_memsz;
					}
				}
			}
			// SYMBOL "_BL_SPACE" Sym { st_name: 5342, st_info: 0x0 LOCAL NOTYPE, st_other: 0 DEFAULT, st_shndx: 65521, st_value: 0x29000, st_size: 0 }
			// The part of the program we copy into the flash image should be
			// of the same size as the space allocated at loader build time.
			let symsz = elf_symbol(&binary, "_BL_SPACE")
				.ok_or(Error::IncompatibleExecutable)?
				.st_value;
			//eprintln!("_BL_SPACE: {:x?}", symsz);
			if totalsz != symsz as usize {
				return Err(Error::IncompatibleExecutable);
			}
			sz = totalsz;

			// These symbols have been embedded into the loader to serve as
			// checks in this exact application.
			let sloader = elf_symbol(&binary, "__sloader")
				.ok_or(Error::IncompatibleExecutable)?
				.st_value;
			//eprintln!("__sloader: {:x?}", sloader);
			if sloader !=
				destination_origin.ok_or(
					Error::IncompatibleExecutable,
				)? {
				return Err(Error::IncompatibleExecutable);
			}

			let eloader = elf_symbol(&binary, "__eloader")
				.ok_or(Error::IncompatibleExecutable)?
				.st_value;
			//eprintln!("__eloader: {:x?}", eloader);
			if eloader != last_vaddr {
				return Err(Error::IncompatibleExecutable);
			}

			// The entry point (reset vector) must be 0x10 bytes below the
			// end of a (real-mode) segment--and that segment must begin at the end
			// of the loaded program.  See AMD pub 55758 sec. 4.3 item 4.
			if binary.header.e_entry !=
				last_vaddr.checked_sub(0x10).ok_or(
					Error::IncompatibleExecutable,
				)? {
				return Err(Error::IncompatibleExecutable);
			}
			if last_vaddr & 0xffff != 0 {
				return Err(Error::IncompatibleExecutable);
			}
		}
		_ => {
			destination_origin = Some(0x8000_0000u64
				.checked_sub(buffer.len() as u64)
				.ok_or(Error::ImageTooBig)?);
			iov = Box::new(&buffer.as_slice()[..]) as Box<dyn Read>;
			sz = buffer.len();
		}
	}

	if destination_origin == None {
		eprintln!("Warning: No destination in RAM specified for Reset image.");
	}

	let beginning = ErasableLocation::try_from(
		static_config::RESET_IMAGE_BEGINNING,
	)
	.map_err(|_| Error::Efs(amd_efs::Error::Misaligned))?;
	// round up:
	let sz2 = if sz % ERASABLE_BLOCK_SIZE == 0 {
		sz
	} else {
		sz.checked_add(ERASABLE_BLOCK_SIZE - (sz % ERASABLE_BLOCK_SIZE))
			.ok_or_else(|| Error::ImageTooBig)?
	};
	let end = beginning
		.advance(sz2)
		.map_err(|_| Error::Efs(amd_efs::Error::Misaligned))?;
	if Location::from(end) > static_config::RESET_IMAGE_END {
		return Err(Error::ImageTooBig);
	}
	let mut entry = BhdDirectoryEntry::new_payload(
		bhd_directory.directory_address_mode(),
		BhdDirectoryEntryType::Bios,
		Some(sz.try_into().unwrap()),
		Some(ValueOrLocation::EfsRelativeOffset(beginning.into())),
		destination_origin,
	)
	.unwrap()
	.with_reset_image(true)
	.with_copy_image(true)
	.build();
	bhd_directory.add_from_reader_with_custom_size(
		Some(beginning),
		&mut entry,
		&mut iov,
	)?;
	Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(
	name = "amd-host-image-builder",
	about = "Build host flash image for AMD Zen CPUs."
)]
struct Opts {
	#[structopt(short = "o", long = "output-file", parse(from_os_str))]
	output_filename: PathBuf,

	#[structopt(short = "r", long = "reset-image", parse(from_os_str))]
	reset_image_filename: PathBuf,

	#[structopt(short = "c", long = "config", parse(from_os_str))]
	efs_configuration_filename: PathBuf,

	#[structopt(short = "B", long = "blobdir", parse(from_os_str))]
	blobdirs: Vec<PathBuf>,

	#[structopt(short = "v", long = "verbose")]
	verbose: bool,
}

fn run() -> std::io::Result<()> {
	//let args: Vec<String> = env::args().collect();
	let opts = Opts::from_args();

	let filename = &opts.output_filename;
	let efs_to_io_error = |e| {
		std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("EFS error: {:?} in file {:?}", e, filename),
		)
	};
	let flash_to_io_error = |e| {
		std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("Flash error: {:?} in file {:?}", e, filename),
		)
	};
	let apcb_to_io_error = |e| {
		std::io::Error::new(
			std::io::ErrorKind::Other,
			format!(
				"APCB error: {:?} in file {:?}",
				e, opts.efs_configuration_filename
			),
		)
	};
	let json5_to_io_error = |e: json5::Error| {
		match e {
			json5::Error::Message { ref msg, ref location } => {
				std::io::Error::new(
					std::io::ErrorKind::Other,
					format!(
						"JSON5 error: {} in file {:?} at {}",
						msg,
						opts.efs_configuration_filename,
						match location {
							None => "unknown location".to_owned(),
							Some(x) => format!("{:?}", x),
						}
					)
			        )
			},
		}
	};
	let amd_host_image_builder_config_error_to_io_error =
		|e: amd_host_image_builder_config::Error| {
			std::io::Error::new(
				std::io::ErrorKind::Other,
				format!(
					"Config error: {:?} in file {:?}",
					e, opts.reset_image_filename
				),
			)
		};

	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.open(filename)?;
	file.set_len(static_config::IMAGE_SIZE.into())?;
	let mut storage = FlashImage::new(file, &filename);
	let mut position: AlignedLocation =
		0.try_into().map_err(flash_to_io_error)?;
	while Location::from(position) < static_config::IMAGE_SIZE {
		FlashWrite::<ERASABLE_BLOCK_SIZE>::erase_block(
			&mut storage,
			position,
		)
		.map_err(flash_to_io_error)?;
		position = position
			.advance(ERASABLE_BLOCK_SIZE)
			.map_err(flash_to_io_error)?;
	}
	assert!(Location::from(position) == static_config::IMAGE_SIZE);
	let path = Path::new(&opts.efs_configuration_filename);
	let data = std::fs::read_to_string(path)?;
	let config: SerdeConfig =
		json5::from_str(&data).map_err(json5_to_io_error)?;

	let SerdeConfig {
		processor_generation,
		spi_mode_bulldozer,
		spi_mode_zen_naples,
		spi_mode_zen_rome,
		psp,
		bhd,
	} = config;
	let host_processor_generation = processor_generation;
	let mut efs = match Efs::<_, ERASABLE_BLOCK_SIZE>::create(
		storage,
		host_processor_generation,
		static_config::EFH_BEGINNING(host_processor_generation),
		Some(static_config::IMAGE_SIZE),
	) {
		Ok(efs) => efs,
		Err(e) => {
			eprintln!("Error on creation: {:?}", e);
			std::process::exit(1);
		}
	};
	efs.set_spi_mode_bulldozer(spi_mode_bulldozer);
	efs.set_spi_mode_zen_naples(spi_mode_zen_naples);
	efs.set_spi_mode_zen_rome(spi_mode_zen_rome);
	let blobdirs = &opts.blobdirs;
	let resolve_blob =
		|blob_filename: PathBuf| -> std::io::Result<PathBuf> {
			if blob_filename.has_root() {
				if blob_filename.exists() {
					Ok(blob_filename.to_path_buf())
				} else {
					Err(std::io::Error::new(
		                        std::io::ErrorKind::Other,
		                        format!("Blob read error: Could not find file {:?}", blob_filename),
		                ))
				}
			} else {
				for blobdir in blobdirs {
					let fullname =
						blobdir.join(&blob_filename);
					if fullname.exists() {
						if opts.verbose {
							eprintln!("Info: Using blob {:?}", fullname);
						}
						return Ok(fullname);
					}
				}
				Err(std::io::Error::new(
	                        std::io::ErrorKind::Other,
	                        format!("Blob read error: Could not find file {:?} \
(neither directly nor in any of the directories {:?})", blob_filename, blobdirs),
	                ))
			}
		};

	let mut psp_directory = efs
		.create_psp_directory(
			AlignedLocation::try_from(static_config::PSP_BEGINNING)
				.map_err(flash_to_io_error)?,
			AlignedLocation::try_from(static_config::PSP_END)
				.map_err(flash_to_io_error)?,
			AddressMode::EfsRelativeOffset,
		)
		.map_err(efs_to_io_error)?;
	let mut abl0_version: Option<u32> = None;
	let mut abl0_version_found = false;
	match psp {
		SerdePspDirectoryVariant::PspDirectory(serde_psp_directory) => {
			for entry in serde_psp_directory.entries {
				let mut raw_entry = PspDirectoryEntry::try_from_with_context(psp_directory.directory_address_mode(), &entry.target).unwrap();
				//eprintln!("{:?}", entry.target.attrs);
				let blob_slot_settings = entry.target.blob;
				// blob_slot_settings is optional.
				// Value means no blob slot settings allowed

				match entry.source {
					SerdePspEntrySource::Value(x) => {
						// FIXME: assert!(blob_slot_settings.is_none()); fails for some reason
						// DirectoryRelativeOffset is the one that can always be overridden
						raw_entry.set_source(AddressMode::DirectoryRelativeOffset, ValueOrLocation::Value(x)).unwrap();
						psp_directory
							.add_value_entry(
								&mut raw_entry,
							)
							.map_err(
								efs_to_io_error,
							)?;
					}
					SerdePspEntrySource::BlobFile(
						blob_filename,
					) => {
						let flash_location =
							blob_slot_settings.and_then(|x| x.flash_location);
						let x: Option<Location> =
							flash_location.map(
								|x| {
									x.try_into().unwrap() // infallible
								},
							);
						let blob_filename = resolve_blob(blob_filename)?;
						if raw_entry.type_() == PspDirectoryEntryType::Abl0 {
							let new_abl0_version = psp_file_version(&blob_filename);
							if !abl0_version_found {
								abl0_version = new_abl0_version;
								abl0_version_found = true
							}
							// For now, we do not support different ABL0 versions in the same image.
							if new_abl0_version != abl0_version {
								return Err(
									std::io::Error::new(
							                        std::io::ErrorKind::Other,
										"different ABL0 versions in the same flash are unsupported"
								))
							}
						}
						// already done by try_from: raw_entry.set_size(size);
						psp_entry_add_from_file(
							&mut psp_directory,
							match x {
								Some(x) => Some(x.try_into()
									.map_err(flash_to_io_error)?),
								None => None
							},
							&mut raw_entry,
							blob_filename,
						)
							.map_err(efs_to_io_error)?;
					}
				}
			}
		}
		_ => {
			todo!();
		}
	}

	if let Some(abl0_version) = abl0_version {
		if opts.verbose {
			// See AgesaBLReleaseNotes.txt, section "ABL Version String"
			println!("Info: Abl0 version: 0x{:x}", abl0_version)
		}
	}

	let mut bhd_directory = efs
		.create_bhd_directory(
			AlignedLocation::try_from(static_config::BHD_BEGINNING)
				.map_err(flash_to_io_error)?,
			AlignedLocation::try_from(static_config::BHD_END)
				.map_err(flash_to_io_error)?,
			AddressMode::EfsRelativeOffset,
		)
		.map_err(efs_to_io_error)?;

	match bhd {
		SerdeBhdDirectoryVariant::BhdDirectory(serde_bhd_directory) => {
			for entry in serde_bhd_directory.entries {
				let mut raw_entry = BhdDirectoryEntry::try_from_with_context(bhd_directory.directory_address_mode(), &entry.target).unwrap();
				let blob_slot_settings = entry.target.blob;
				let flash_location = blob_slot_settings.and_then(|x| x.flash_location);
				let x: Option<Location> = flash_location
					.map(|x| x.try_into().unwrap()); // infallible

				// done by try_from: raw_entry.set_destination_location(ram_destination_address);
				// done by try_from: raw_entry.set_size(size);

				match entry.source {
					SerdeBhdSource::BlobFile(
						blob_filename,
					) => {
						bhd_entry_add_from_file(
							&mut bhd_directory,
							match x {
								Some(x) => Some(x.try_into()
									.map_err(flash_to_io_error)?),
								None => None
							},
							&mut raw_entry,
							resolve_blob(blob_filename)?,
						).map_err(efs_to_io_error)?;
					}
					SerdeBhdSource::ApcbJson(apcb) => {
						// XXX: blob_slot_settings.size
						// is ignored since the Apcb has
						// already been deserialized.

						// Note: We need to do this
						// manually because validation
						// needs ABL0_VERSION.
						apcb.validate(abl0_version)
							.map_err(apcb_to_io_error)?;
						let buf = apcb
							.save_no_inc()
							.map_err(
							apcb_to_io_error,
						)?;
						let mut bufref = buf.as_ref();
						if raw_entry.size().is_none() {
							raw_entry.set_size(
								Some(bufref
									.len()
									.try_into(
									)
									.unwrap(
									)),
							);
						};
						bhd_directory
							.add_from_reader_with_custom_size(
								x.and_then(
									|y| {
										y.try_into().ok()
									},
								),
								&mut raw_entry,
								&mut bufref,
							)
							.map_err(
								efs_to_io_error,
							)?;
					}
				}
			}
		}
		_ => {
			todo!();
		}
	}

	bhd_directory
		.add_apob_entry(BhdDirectoryEntryType::Apob, 0x400_0000)
		.map_err(efs_to_io_error)?;

	bhd_directory_add_reset_image(
		&mut bhd_directory,
		&opts.reset_image_filename,
	)
	.map_err(amd_host_image_builder_config_error_to_io_error)?;

	Ok(())
}

fn main() -> std::io::Result<()> {
	run().map_err(|e| {
		eprintln!("Error: {}", e);
		std::process::exit(1);
	})
}
