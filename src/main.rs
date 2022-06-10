use amd_efs::{
	BhdDirectory, BhdDirectoryEntryAttrs, BhdDirectoryEntryType, Efs,
	ProcessorGeneration, PspDirectory, PspDirectoryEntryAttrs,
	AddressMode,
};
use amd_host_image_builder_config::{
	SerdePspDirectoryVariant,
	SerdeBhdDirectoryVariant,
	SerdeBhdSource,
	SerdePspEntrySource,
	Result,
	Error,
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

use amd_host_image_builder_config::SerdeConfig;
use amd_flash::{ErasableLocation, FlashRead, FlashWrite, Location};
use amd_efs::DirectoryFrontend;

#[test]
fn test_bitfield_serde() {
	let config = r#"{
	"max_size": 2,
	"base_address": 3,
	"address_mode": "PhysicalAddress"
}"#;
	use amd_efs::DirectoryAdditionalInfo;
	let result: DirectoryAdditionalInfo = serde_yaml::from_str(config).unwrap();
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

const IMAGE_SIZE: u32 = 16 * 1024 * 1024;
const ERASABLE_BLOCK_SIZE: usize = 0x1000;
type AlignedLocation = ErasableLocation<ERASABLE_BLOCK_SIZE>;

fn psp_entry_add_from_file_with_custom_size(
	directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &PspDirectoryEntryAttrs,
	size: usize,
	source_filename: &Path,
) -> amd_efs::Result<()> {
	//eprintln!("FILE {:?}", source_filename);
	let file = File::open(source_filename).unwrap();
	let mut source = BufReader::new(file);

	directory.add_from_reader_with_custom_size(
		payload_position,
		attrs,
		size,
		&mut source,
		None,
	)
}

fn psp_entry_add_from_file(
	directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &PspDirectoryEntryAttrs,
	source_filename: PathBuf,
	target_size: Option<usize>,
) -> amd_efs::Result<()> {
	let source_filename = source_filename.as_path();
	let file = match File::open(source_filename) {
		Ok(f) => f,
		Err(e) => {
			panic!("Could not open file {:?}: {}", source_filename, e);
		}
	};
	let filesize: usize = file.metadata().unwrap().len().try_into().unwrap();
	let size = match target_size {
		Some(x) => {
			if filesize > x {
				panic!("Configuration specifies slot size {} but contents {:?} have size {}. The contents do not fit.", x, source_filename, filesize);
			}
			x
		}
		None => {
			filesize
		}
	};
	psp_entry_add_from_file_with_custom_size(
		directory,
		payload_position,
		attrs,
		size,
		&source_filename,
	)
}

fn bhd_entry_add_from_file_with_custom_size(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &BhdDirectoryEntryAttrs,
	size: usize,
	source_filename: &Path,
	ram_destination_address: Option<u64>,
) -> amd_efs::Result<()> {
	let file = File::open(source_filename).unwrap();
	let mut reader = BufReader::new(file);

	directory.add_from_reader_with_custom_size(
		payload_position,
		attrs,
		size,
		&mut reader,
		ram_destination_address,
	)
}

fn bhd_entry_add_from_file(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &BhdDirectoryEntryAttrs,
	source_filename: PathBuf,
	ram_destination_address: Option<u64>,
	target_size: Option<usize>,
) -> amd_efs::Result<()> {
	let source_filename = source_filename.as_path();
	let file = match File::open(source_filename) {
		Ok(f) => f,
		Err(e) => {
			panic!("Could not open file {:?}: {}", source_filename, e);
		}
	};
	let filesize: usize = file.metadata().unwrap().len().try_into().unwrap();
	let size = match target_size {
		Some(x) => {
			if filesize > x {
				eprintln!("Configuration specifies slot size {} but contents {:?} have size {}. The contents to dot fit.", x, source_filename, filesize);
			}
			x
		}
		None => {
			filesize
		}
	};
	bhd_entry_add_from_file_with_custom_size(
		directory,
		payload_position,
		attrs,
		size,
		&source_filename,
		ram_destination_address,
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
		static_config::RESET_IMAGE_BEGINNING
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
	bhd_directory.add_from_reader_with_custom_size(
		Some(beginning),
		&BhdDirectoryEntryAttrs::builder()
			.with_type_(BhdDirectoryEntryType::Bios)
			.with_reset_image(true)
			.with_copy_image(true)
			.build(),
		sz,
		&mut iov,
		destination_origin,
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
}

//fn read_config_from_file<P: AsRef<Path> + std::fmt::Debug>(path: P) -> Result<SerdeConfig<'_>> { // , Box<Error>
//	//eprintln!("config_from_file {:?}", path);
//	let file = File::open(path).unwrap();
//	let reader = BufReader::new(file);
//	let result = serde_yaml::from_reader(reader).unwrap();
//	Ok(result)
//}

fn main() -> std::io::Result<()> {
	//let args: Vec<String> = env::args().collect();
	let opts = Opts::from_args();

	let filename = &opts.output_filename;
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.open(filename)?;
	file.set_len(IMAGE_SIZE.into())?;
	let mut storage = FlashImage::new(file, &filename);
	let mut position: AlignedLocation = 0.try_into().unwrap();
	while Location::from(position) < IMAGE_SIZE {
		FlashWrite::<ERASABLE_BLOCK_SIZE>::erase_block(
			&mut storage,
			position,
		)
		.unwrap();
		position = position.advance(ERASABLE_BLOCK_SIZE).unwrap();
	}
	assert!(Location::from(position) == IMAGE_SIZE);
	let path = Path::new(&opts.efs_configuration_filename);
	//let reader = BufReader::new(file);
        let data = std::fs::read_to_string(path)?;
        let config: SerdeConfig = serde_json::from_str(&data).unwrap();
        //let config = serde_yaml::from_reader(reader).unwrap();

	//let config = read_config_from_file(path).unwrap();
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
		Some(IMAGE_SIZE),
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
	let firmware_blob_directory_name = match host_processor_generation {
		ProcessorGeneration::Milan => {
			Path::new("amd-firmware").join("milan")
		}
		ProcessorGeneration::Rome => {
			Path::new("amd-firmware").join("rome")
		}
		ProcessorGeneration::Naples => {
			Path::new("amd-firmware").join("naples")
		}
	};

	let mut psp_directory = efs
		.create_psp_directory(
			AlignedLocation::try_from(static_config::PSP_BEGINNING).unwrap(),
			AlignedLocation::try_from(static_config::PSP_END).unwrap(),
			AddressMode::EfsRelativeOffset,
		)
		.unwrap();
	match psp {
		SerdePspDirectoryVariant::PspDirectory(serde_psp_directory) => {
			for entry in serde_psp_directory.entries {
				//eprintln!("{:?}", entry.target.attrs);
				let blob_slot_settings = entry.target.blob;
				// blob_slot_settings is optional.
				// Value means no blob slot settings allowed

				match entry.source {
					SerdePspEntrySource::Value(x) => {
						// FIXME: assert!(blob_slot_settings.is_none()); fails for some reason
						psp_directory.add_value_entry(
							&entry.target.attrs,
							x, // TODO: Nicer type.
						).unwrap();
					}
					SerdePspEntrySource::BlobFile(blob_filename) => {
						let (flash_location, size) = match blob_slot_settings {
							Some(x) => (x.flash_location, x.size),
							None => (None, None)
						};
						let x: Option<Location> = flash_location.map(|x| x.try_into().unwrap());
						psp_entry_add_from_file(
							&mut psp_directory,
							match x {
								Some(x) => Some(x.try_into().unwrap()),
								None => None
							},
							&entry.target.attrs,
							firmware_blob_directory_name.join(blob_filename),
							size.map(|x| x as usize),
						).unwrap();
					},
				}
			}
		}
		_ => {
			todo!();
		}
	}

	let mut bhd_directory = efs
		.create_bhd_directory(
			AlignedLocation::try_from(static_config::BHD_BEGINNING).unwrap(),
			AlignedLocation::try_from(static_config::BHD_END)
				.unwrap(),
			AddressMode::EfsRelativeOffset,
		)
		.unwrap();

	match bhd {
		SerdeBhdDirectoryVariant::BhdDirectory(serde_bhd_directory) => {
			for entry in serde_bhd_directory.entries {
				let blob_slot_settings = entry.target.blob;
				let (flash_location, size, ram_destination_address) = match blob_slot_settings {
					Some(x) => (x.flash_location, x.size, x.ram_destination_address),
					None => (None, None, None)
				};
				let x: Option<Location> = flash_location.map(|x| x.try_into().unwrap());
				match entry.source {
					SerdeBhdSource::BlobFile(blob_filename) => {
						bhd_entry_add_from_file(
							&mut bhd_directory,
							match x {
								Some(x) => Some(x.try_into().unwrap()),
								None => None
							},
							&entry.target.attrs,
							firmware_blob_directory_name.join(blob_filename),
							ram_destination_address,
							size.map(|x| x as usize),
						).unwrap();
					}
					SerdeBhdSource::ApcbJson(apcb) => {
						let buf = apcb.save_no_inc().unwrap();
						let mut bufref = buf.as_ref();
						bhd_directory.add_from_reader_with_custom_size(
							x.map(|y| y.try_into().unwrap()),
							&entry.target.attrs,
							size.unwrap_or(bufref.len().try_into().unwrap()).try_into().unwrap(),
							&mut bufref,
							None,
						).unwrap();
					}
				}
			}
		}
		_ => {
			todo!();
		}
	}

	bhd_directory
		.add_apob_entry(None, BhdDirectoryEntryType::Apob, 0x400_0000)
		.unwrap();

	bhd_directory_add_reset_image(
		&mut bhd_directory,
		&opts.reset_image_filename,
	)
	.unwrap();

	Ok(())
}
