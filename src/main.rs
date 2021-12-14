use amd_efs::{
	BhdDirectory, BhdDirectoryEntryAttrs, BhdDirectoryEntryType, Efs,
	ProcessorGeneration, PspDirectory, PspDirectoryEntryAttrs,
	PspDirectoryEntryType, PspSoftFuseChain,
};
use core::cell::RefCell;
use core::convert::TryFrom;
use core::convert::TryInto;
use core::num::NonZeroU8;
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

use amd_apcb::Apcb;
//use amd_efs::ProcessorGeneration;
use amd_flash::{ErasableLocation, FlashRead, FlashWrite, Location};

#[derive(Debug)]
enum Error {
	Efs(amd_efs::Error),
	IncompatibleExecutable,
	Io(std::io::Error),
	ImageTooBig,
}

type Result<T> = core::result::Result<T, Error>;

impl From<amd_efs::Error> for Error {
	fn from(err: amd_efs::Error) -> Self {
		Self::Efs(err)
	}
}

mod hole;
use hole::Hole;

struct FlashImage {
	file: RefCell<File>,
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
				return Err(amd_flash::Error::Io);
			}
		}
		match file.read_exact(buffer) {
			Ok(()) => Ok(buffer.len()),
			Err(e) => {
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
				return Err(amd_flash::Error::Io);
			}
		}
		match file.read(buffer) {
			Ok(size) => {
				assert!(size == ERASABLE_BLOCK_SIZE);
				Ok(())
			}
			Err(e) => {
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
				return Err(amd_flash::Error::Io);
			}
		}
		match file.write(&(*buffer)[..]) {
			Ok(size) => {
				assert!(size == ERASABLE_BLOCK_SIZE);
				Ok(())
			}
			Err(e) => {
				return Err(amd_flash::Error::Io);
			}
		}
	}
}

impl FlashImage {
	fn new(file: File) -> Self {
		Self {
			file: RefCell::new(file),
		}
	}
}

const IMAGE_SIZE: u32 = 16 * 1024 * 1024;
const ERASABLE_BLOCK_SIZE: usize = 0x1000;
type AlignedLocation = ErasableLocation<ERASABLE_BLOCK_SIZE>;

fn psp_entry_add_from_file(
	directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &PspDirectoryEntryAttrs,
	source_filename: PathBuf,
) -> amd_efs::Result<()> {
	let file = File::open(source_filename).unwrap();
	let size: usize = file.metadata().unwrap().len().try_into().unwrap();
	let mut source = BufReader::new(file);
	directory.add_blob_entry(
		payload_position,
		attrs,
		size.try_into().unwrap(),
		&mut |buf: &mut [u8]| {
			let mut cursor = 0;
			loop {
				let bytes = source
					.read(&mut buf[cursor ..])
					.map_err(|_| amd_efs::Error::Marshal)?;
				if bytes == 0 {
					return Ok(cursor);
				}
				cursor += bytes;
			}
		},
	)?;
	Ok(())
}

//
// The comment in efs.rs:add_payload() states that the function passed into
// directory.add_blob_entry() must not return a result smaller than the length
// of the buffer passed into it unless there are no more contents.  This means
// we cannot expect it to be called repeatedly, which is to say that we must
// loop ourselves until the reader we are given returns no more data.  This
// matters because it is *not* an error for a reader to return less data than
// would have filled the buffer it was given, even if more data might be
// available.
//
fn bhd_entry_add_from_reader_with_custom_size<T>(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &BhdDirectoryEntryAttrs,
	size: usize,
	source: &mut T,
	ram_destination_address: Option<u64>,
) -> amd_efs::Result<()>
where
	T: std::io::Read,
{
	directory.add_blob_entry(
		payload_position,
		attrs,
		size.try_into().unwrap(),
		ram_destination_address,
		&mut |buf: &mut [u8]| {
			let mut cursor = 0;
			loop {
				let bytes = source
					.read(&mut buf[cursor ..])
					.map_err(|_| amd_efs::Error::Marshal)?;
				if bytes == 0 {
					return Ok(cursor);
				}
				cursor += bytes;
			}
		},
	)?;
	Ok(())
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

	bhd_entry_add_from_reader_with_custom_size(
		directory,
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
) -> amd_efs::Result<()> {
	let source_filename = source_filename.as_path();
	let file = File::open(source_filename).unwrap();
	let size: usize = file.metadata().unwrap().len().try_into().unwrap();
	bhd_entry_add_from_file_with_custom_size(
		directory,
		payload_position,
		attrs,
		size,
		&source_filename,
		ram_destination_address,
	)
}

fn psp_directory_add_default_entries(
	psp_directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	firmware_blob_directory_name: &PathBuf,
) -> amd_efs::Result<()> {
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::AblPublicKey),
		firmware_blob_directory_name.join("AblPubKey.bin"), // that was weird: "PspABLFw_gn.stkn", // imm
	)?;

	psp_directory.add_value_entry(
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::PspSoftFuseChain),
		PspSoftFuseChain::new()
			.with_secure_debug_unlock(true)
			.into(),
	)?;

	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new().with_type_(
			PspDirectoryEntryType::SmuOffChipFirmware12,
		),
		firmware_blob_directory_name.join("SmuFirmware2.csbin"),
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new().with_type_(
			PspDirectoryEntryType::PspEarlySecureUnlockDebugImage,
		),
		firmware_blob_directory_name.join("SecureDebugUnlock.sbin"),
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::WrappedIkek),
		firmware_blob_directory_name.join("PspIkek.bin"), // imm
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::PspTokenUnlockData),
		firmware_blob_directory_name.join("SecureEmptyToken.bin"), // imm
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new().with_type_(
			PspDirectoryEntryType::SecurityPolicyBinary,
		),
		firmware_blob_directory_name.join("RsmuSecPolicy.sbin"),
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::Mp5Firmware),
		firmware_blob_directory_name.join("Mp5.csbin"),
	)?;
	psp_entry_add_from_file(
		psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::Abl0),
		firmware_blob_directory_name
			.join("AgesaBootloader_U_prod.csbin"),
	)?;
	Ok(())
}

fn bhd_entry_add_from_file_if_present(
	directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
	attrs: &BhdDirectoryEntryAttrs,
	source_filename: PathBuf,
	ram_destination_address: Option<u64>,
) -> amd_efs::Result<()> {
	if source_filename.as_path().exists() {
		bhd_entry_add_from_file(
			directory,
			payload_position,
			attrs,
			source_filename,
			ram_destination_address,
		)
	} else {
		Ok(())
	}
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

	bhd_entry_add_from_reader_with_custom_size(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::Bios)
			.with_reset_image(true)
			.with_copy_image(true),
		sz,
		&mut iov,
		destination_origin,
	)?;
	Ok(())
}

fn bhd_directory_add_default_entries(
	bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	firmware_blob_directory_name: &PathBuf,
) -> amd_efs::Result<()> {
	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(1)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_1D_Ddr4_Udimm_Imem.csbin"),
		None,
	)?;
	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(1)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_1D_Ddr4_Udimm_Dmem.csbin"),
		None,
	)?;

	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(2)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_1D_Ddr4_Rdimm_Imem.csbin"),
		None,
	)?;
	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(2)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_1D_Ddr4_Rdimm_Dmem.csbin"),
		None,
	)?;

	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(4)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_2D_Ddr4_Udimm_Imem.csbin"),
		None,
	)?;
	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(4)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_2D_Ddr4_Udimm_Dmem.csbin"),
		None,
	)?;

	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(5)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_2D_Ddr4_Rdimm_Imem.csbin"),
		None,
	)?;
	bhd_entry_add_from_file(
		bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(5)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_2D_Ddr4_Rdimm_Dmem.csbin"),
		None,
	)?;
	Ok(())
}

fn bhd_add_apcb(
	processor_generation: ProcessorGeneration,
	bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
	attrs: &BhdDirectoryEntryAttrs,
) -> amd_apcb::Result<()> {
	use amd_apcb::memory::platform_specific_override;
	use amd_apcb::memory::*;
	use amd_apcb::ApcbIoOptions;
	use amd_apcb::BoardInstances;
	use amd_apcb::EntryId;
	use amd_apcb::GroupId;
	use amd_apcb::MemoryEntryId;
	use amd_apcb::PriorityLevel;
	use amd_apcb::PriorityLevels;
	use amd_apcb::{
		BaudRate, BmcGen2TxDeemphasis, BmcLinkSpeed, CcxSevAsidCount,
		ContextType, DfCakeCrcThresholdBounds, DfDramNumaPerSocket,
		DfMemInterleaving, DfMemInterleavingSize, DfPstateModeSelect,
		DfRemapAt1TiB, DfToggle, DfXgmiLinkConfig, DfXgmiTxEqMode,
		DxioPhyParamDc, DxioPhyParamIqofc, DxioPhyParamPole,
		DxioPhyParamVga, EccSymbolSize, FchConsoleOutSuperIoType,
		FchConsoleSerialPort, FchGppClkMap, FchSmbusSpeed,
		GnbSmuDfPstateFclkLimit, MemActionOnBistFailure, MemClockValue,
		MemControllerPmuTrainingMode, MemControllerWritingCrcMode,
		MemDataPoison, MemHealBistEnable, MemHealPprType,
		MemHealTestSelect, MemMaxActivityCount,
		MemMbistAggressorsChannels, MemMbistDataEyeType,
		MemMbistPatternSelect, MemMbistTest, MemMbistTestMode,
		MemNvdimmPowerSource, MemRdimmTimingCmdParLatency,
		MemSelfRefreshExitStaggering, MemThrottleCtrlRollWindowDepth,
		MemTrainingHdtControl, MemTsmeMode, MemUserTimingMode,
		PspEnableDebugMode, SecondPcieLinkMaxPayload,
		SecondPcieLinkSpeed, TokenEntryId, UmaMode, WorkloadProfile,
	};
	let mut buf: [u8; Apcb::MAX_SIZE] = [0xff; Apcb::MAX_SIZE];
	let mut apcb = Apcb::create(
		&mut buf,
		1,                         /*FIXME*/
		&ApcbIoOptions::default(), /*FIXME*/
	)?;
	apcb.insert_group(GroupId::Memory, *b"MEMG")?;
	apcb.insert_struct_array_as_entry::<DimmInfoSmbusElement>(
		EntryId::Memory(MemoryEntryId::DimmInfoSmbus),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			// socket_id, channel_id, dimm_id, dimm_smbus_address, i2c_mux_address=148, mux_control_address=3, mux_channel
			DimmInfoSmbusElement::new_slot(
				0,
				0,
				0,
				160,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				0,
				1,
				162,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				1,
				0,
				164,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				1,
				1,
				166,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				2,
				0,
				168,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				2,
				1,
				170,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				3,
				0,
				172,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				3,
				1,
				174,
				Some(148),
				Some(3),
				Some(128),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				4,
				0,
				160,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				4,
				1,
				162,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				5,
				0,
				164,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				5,
				1,
				166,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				6,
				0,
				168,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				6,
				1,
				170,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				7,
				0,
				172,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				0,
				7,
				1,
				174,
				Some(148),
				Some(3),
				Some(64),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				0,
				0,
				160,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				0,
				1,
				162,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				1,
				0,
				164,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				1,
				1,
				166,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				2,
				0,
				168,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				2,
				1,
				170,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				3,
				0,
				172,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				3,
				1,
				174,
				Some(148),
				Some(3),
				Some(32),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				4,
				0,
				160,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				4,
				1,
				162,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				5,
				0,
				164,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				5,
				1,
				166,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				6,
				0,
				168,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				6,
				1,
				170,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				7,
				0,
				172,
				Some(148),
				Some(3),
				Some(16),
			)?,
			DimmInfoSmbusElement::new_slot(
				1,
				7,
				1,
				174,
				Some(148),
				Some(3),
				Some(16),
			)?,
		],
	)?;
	// &[&dyn SequenceElementAsBytes]
	use platform_specific_override::ChannelIds;
	use platform_specific_override::DimmSlots;
	use platform_specific_override::SocketIds;
	apcb.insert_struct_sequence_as_entry(
		EntryId::Memory(MemoryEntryId::PlatformSpecificOverride),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			&platform_specific_override::MemclkMap::new(
				SocketIds::ALL,
				ChannelIds::Any,
				[0, 1, 2, 3, 0, 0, 0, 0],
			)?,
			&platform_specific_override::CkeTristateMap::new(
				SocketIds::ALL,
				ChannelIds::Any,
				DimmSlots::Any,
				[0, 1, 2, 3],
			)?,
			&platform_specific_override::OdtTristateMap::new(
				SocketIds::ALL,
				ChannelIds::Any,
				DimmSlots::Any,
				[0, 1, 2, 3],
			)?,
			&platform_specific_override::CsTristateMap::new(
				SocketIds::ALL,
				ChannelIds::Any,
				DimmSlots::Any,
				[0, 1, 2, 3, 0, 0, 0, 0],
			)?,
			&platform_specific_override::MaxDimmsPerChannel::new(
				SocketIds::ALL,
				ChannelIds::Any,
				1,
			)?, // FIXME check orig
			&platform_specific_override::MaxChannelsPerSocket::new(
				SocketIds::ALL,
				8,
			)?,
		],
	)?;
	match processor_generation {
		ProcessorGeneration::Naples => {
			panic!("not supported");
		}
		ProcessorGeneration::Rome => {
			// PPR 12.7.2.2 DRAM ODT Pin Control
			apcb.insert_struct_array_as_entry::<Ddr4OdtPatElement>(
				EntryId::Memory(
					MemoryEntryId::PsRdimmDdr4OdtPat,
				),
				0,
				BoardInstances::all(),
				PriorityLevels::from_level(
					PriorityLevel::Normal,
				),
				&[
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new()
							.with_writing_pattern(2)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(8)
							.with_reading_pattern(
								0,
							),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(4)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(2)
							.with_reading_pattern(
								2,
							),
						OdtPatPatterns::new(),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(2)
							.with_reading_pattern(
								2,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(2)
							.with_reading_pattern(
								2,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(
								0xa,
							)
							.with_reading_pattern(
								0xa,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(
								0xa,
							)
							.with_reading_pattern(
								0xa,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(5)
							.with_reading_pattern(
								5,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(5)
							.with_reading_pattern(
								5,
							),
					),
				],
			)?;
		}
		ProcessorGeneration::Milan => {
			// PPR 12.7.2.2 DRAM ODT Pin Control
			apcb.insert_struct_array_as_entry::<Ddr4OdtPatElement>(
				EntryId::Memory(
					MemoryEntryId::PsRdimmDdr4OdtPat,
				),
				0,
				BoardInstances::all(),
				PriorityLevels::from_level(
					PriorityLevel::Normal,
				),
				&[
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new()
							.with_writing_pattern(4)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(8)
							.with_reading_pattern(
								0,
							),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_unpopulated(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(2)
							.with_reading_pattern(
								0,
							),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(4)
							.with_reading_pattern(
								4,
							),
						OdtPatPatterns::new(),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_single_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(4)
							.with_reading_pattern(
								4,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(4)
							.with_reading_pattern(
								4,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(1)
							.with_reading_pattern(
								1,
							),
						OdtPatPatterns::new(),
					),
					Ddr4OdtPatElement::new(
						Ddr4OdtPatDimmRankBitmaps::new(
						)
						.with_dimm1(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						)
						.with_dimm0(
							Ddr4DimmRanks::new()
								.with_dual_rank(
									true,
								),
						),
						OdtPatPatterns::new()
							.with_writing_pattern(
								0xc,
							)
							.with_reading_pattern(
								0xc,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(
								0xc,
							)
							.with_reading_pattern(
								0xc,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(3)
							.with_reading_pattern(
								3,
							),
						OdtPatPatterns::new()
							.with_writing_pattern(3)
							.with_reading_pattern(
								3,
							),
					),
				],
			)?;
		}
		_ => {
			todo!();
		}
	}
	let u = Ddr4DimmRanks::new().with_unpopulated(true);
	let s = Ddr4DimmRanks::new().with_single_rank(true);
	let d = Ddr4DimmRanks::new().with_dual_rank(true);
	let sd = Ddr4DimmRanks::new()
		.with_single_rank(true)
		.with_dual_rank(true); // s|d
	apcb.insert_struct_array_as_entry::<RdimmDdr4CadBusElement>(
		EntryId::Memory(MemoryEntryId::PsRdimmDdr4CadBus),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			// dimm_slots_per_channel, ddr_rates, dimm0_ranks, dimm1_ranks, address_command_control
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr1600(true),
				sd,
				u,
				0x393939,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr1866(true),
				sd,
				u,
				0x373737,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr2133(true),
				sd,
				u,
				0x353535,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr2400(true),
				sd,
				u,
				0x333333,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr2667(true),
				sd,
				u,
				0x313131,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr2933(true),
				sd,
				u,
				0x2f2f2f,
			)?,
			RdimmDdr4CadBusElement::new(
				1,
				DdrRates::new().with_ddr3200(true),
				sd,
				u,
				0x2d2d2d,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1600(true),
				u,
				sd,
				0x393939,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1600(true),
				sd,
				u,
				0x393939,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1600(true),
				sd,
				sd,
				0x353939,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1866(true),
				u,
				sd,
				0x373737,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1866(true),
				sd,
				u,
				0x373737,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1866(true),
				s,
				s,
				0x333939,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1866(true),
				s,
				d,
				0x333737,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr1866(true),
				d,
				sd,
				0x333737,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2133(true),
				u,
				sd,
				0x353535,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2133(true),
				sd,
				u,
				0x353535,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2133(true),
				sd,
				sd,
				0x313535,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2400(true),
				u,
				sd,
				0x333333,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2400(true),
				sd,
				u,
				0x333333,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2400(true),
				sd,
				sd,
				0x2f3333,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2667(true),
				u,
				sd,
				0x313131,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2667(true),
				sd,
				u,
				0x313131,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2667(true),
				sd,
				sd,
				0x2d3131,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2933(true),
				u,
				sd,
				0x2f2f2f,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2933(true),
				sd,
				u,
				0x2f2f2f,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr2933(true),
				sd,
				sd,
				0x2c2f2f,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr3200(true),
				u,
				sd,
				0x2d2d2d,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr3200(true),
				sd,
				u,
				0x2d2d2d,
			)?,
			RdimmDdr4CadBusElement::new(
				2,
				DdrRates::new().with_ddr3200(true),
				sd,
				sd,
				0x2a2d2d,
			)?,
		],
	)?;
	let ddr_rates = DdrRates::new()
		.with_ddr3200(true)
		.with_ddr2933(true)
		.with_ddr2667(true)
		.with_ddr2400(true)
		.with_ddr2133(true)
		.with_ddr1866(true)
		.with_ddr1600(true);
	apcb.insert_struct_array_as_entry::<Ddr4DataBusElement>(
		EntryId::Memory(MemoryEntryId::PsRdimmDdr4DataBus),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			Ddr4DataBusElement::new(
				1,
				ddr_rates,
				s,
				u,
				RttNom::Off,
				RttWr::Off,
				RttPark::_48Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				1,
				ddr_rates,
				d,
				u,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				93,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				u,
				s,
				RttNom::Off,
				RttWr::Off,
				RttPark::_48Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				u,
				d,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				93,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				s,
				u,
				RttNom::Off,
				RttWr::Off,
				RttPark::_48Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				s,
				s,
				RttNom::Off,
				RttWr::_80Ohm,
				RttPark::_34Ohm,
				104,
				VrefDq::Range1(VrefDqRange1::_78_85P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				s,
				d,
				RttNom::_34Ohm,
				RttWr::_120Ohm,
				RttPark::_240Ohm,
				103,
				VrefDq::Range1(VrefDqRange1::_80_80P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				d,
				u,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				93,
				VrefDq::Range1(VrefDqRange1::_74_95P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				d,
				s,
				RttNom::_34Ohm,
				RttWr::_120Ohm,
				RttPark::_240Ohm,
				103,
				VrefDq::Range1(VrefDqRange1::_80_80P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				d,
				d,
				RttNom::_60Ohm,
				RttWr::_120Ohm,
				RttPark::_240Ohm,
				106,
				VrefDq::Range1(VrefDqRange1::_79_50P),
			)?,
		],
	)?;
	let one_dimm = DimmsPerChannel::Specific(
		DimmsPerChannelSelector::new().with_one_dimm(true),
	);
	let two_dimms = DimmsPerChannel::Specific(
		DimmsPerChannelSelector::new().with_two_dimms(true),
	);
	let unsupported_speed = match processor_generation {
		ProcessorGeneration::Rome => DdrSpeed::UnsupportedRome,
		_ => DdrSpeed::UnsupportedMilan,
	};
	apcb.insert_struct_array_as_entry::<MaxFreqElement>(
		EntryId::Memory(MemoryEntryId::PsRdimmDdr4MaxFreq),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			MaxFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				1,
				0,
				0,
				DdrSpeed::Ddr3200,
			),
			MaxFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				1,
				0,
				0,
				DdrSpeed::Ddr3200,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				2,
				0,
				0,
				DdrSpeed::Ddr2933,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				1,
				1,
				0,
				DdrSpeed::Ddr2933,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				0,
				2,
				0,
				DdrSpeed::Ddr2933,
			),
		],
	)?;
	apcb.insert_struct_array_as_entry::<StretchFreqElement>(
		EntryId::Memory(MemoryEntryId::PsRdimmDdr4StretchFreq),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			StretchFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				1,
				0,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				1,
				0,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				2,
				0,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				1,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				0,
				2,
				0,
				DdrSpeed::Ddr3200,
			),
		],
	)?;
	apcb.insert_struct_array_as_entry::<MaxFreqElement>(
		EntryId::Memory(MemoryEntryId::Ps3dsRdimmDdr4MaxFreq),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			MaxFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr2933,
			),
			MaxFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				0,
				2,
				0,
				DdrSpeed::Ddr2667,
			),
		],
	)?;
	apcb.insert_struct_array_as_entry::<StretchFreqElement>(
		EntryId::Memory(MemoryEntryId::Ps3dsRdimmDdr4StretchFreq),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			StretchFreqElement::new(
				unsupported_speed,
				one_dimm,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				1,
				0,
				1,
				0,
				DdrSpeed::Ddr3200,
			),
			StretchFreqElement::new(
				unsupported_speed,
				two_dimms,
				2,
				0,
				2,
				0,
				DdrSpeed::Ddr3200,
			),
		],
	)?;
	apcb.insert_struct_array_as_entry::<Ddr4DataBusElement>(
		EntryId::Memory(MemoryEntryId::Ps3dsRdimmDdr4DataBus),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[
			Ddr4DataBusElement::new(
				1,
				ddr_rates,
				d,
				u,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_71_70P),
			)?, // FIXME check ddr_rates
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				u,
				d,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_71_70P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				d,
				u,
				RttNom::_60Ohm,
				RttWr::Off,
				RttPark::_240Ohm,
				91,
				VrefDq::Range1(VrefDqRange1::_71_70P),
			)?,
			Ddr4DataBusElement::new(
				2,
				ddr_rates,
				d,
				d,
				RttNom::_60Ohm,
				RttWr::_120Ohm,
				RttPark::_240Ohm,
				104,
				VrefDq::Range1(VrefDqRange1::_77_55P),
			)?,
		],
	)?;

	let console_out = AblConsoleOutControl::new()
		.with_enable_console_logging(true)
		.with_enable_mem_flow_logging(true)
		.with_enable_mem_setreg_logging(true)
		.with_enable_mem_getreg_logging(false)
		.with_enable_mem_status_logging(true)
		.with_enable_mem_pmu_logging(true)
		.with_enable_mem_pmu_sram_read_logging(false)
		.with_enable_mem_pmu_sram_write_logging(false)
		.with_enable_mem_test_verbose_logging(false)
		.with_enable_mem_basic_output_logging(true);
	apcb.insert_struct_entry::<ConsoleOutControl>(
		EntryId::Memory(MemoryEntryId::ConsoleOutControl),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		// TODO: nicer?
		&ConsoleOutControl::new(
			console_out,
			AblBreakpointControl::new(false, false),
		),
		&[],
	)?;
	match processor_generation {
		ProcessorGeneration::Naples => {
			// ?
		}
		ProcessorGeneration::Milan => {
			apcb.insert_struct_entry::<ErrorOutControl116>(
				EntryId::Memory(MemoryEntryId::ErrorOutControl),
				0,
				BoardInstances::all(),
				PriorityLevels::from_level(
					PriorityLevel::Normal,
				),
				&ErrorOutControl116::new()
					.with_enable_error_reporting(false)
					.with_error_reporting_gpio(Some(
						Gpio::new(85, 1, 192),
					))
					.with_input_port(0x84)
					.with_input_port_size(PortSize::_32Bit)
					.with_clear_acknowledgement(false)
					.with_enable_heart_beat(false)
					.with_enable_error_reporting_beep_codes(
						false,
					)
					.with_stop_on_first_fatal_error(false)
					.with_enable_error_reporting_gpio(
						false,
					), // FIXME add values (which have fine defaults) eventually: enable_error_reporting, enable_error_reporting_gpio, enable_error_reporting_beep_codes, enable_using_handshake, input_port: 132.into(), input_port, output_delay, output_port
				// FIXME: stop_on_first_fatal_error: false.into(), input_port_size: 4.into(), output_port_size: 4.into(), input_port_type: 6.into(), output_port_type: 6.into(), clear_acknowledgement: false.into(), error_reporting_gpio: Gpio { pin: 85, iomux_control: 1, bank_control: 192 }, enable_heart_beat: false.into() }
				&[],
			)?;
		}
		ProcessorGeneration::Rome => {
			apcb.insert_struct_entry::<ErrorOutControl112>(
				EntryId::Memory(MemoryEntryId::ErrorOutControl),
				0,
				BoardInstances::all(),
				PriorityLevels::from_level(
					PriorityLevel::Normal,
				),
				&ErrorOutControl112::new()
					.with_enable_error_reporting(false)
					.with_error_reporting_gpio(Some(
						Gpio::new(85, 1, 192),
					))
					.with_input_port(0x84)
					.with_input_port_size(PortSize::_32Bit)
					.with_clear_acknowledgement(false)
					.with_enable_heart_beat(false)
					.with_enable_error_reporting_beep_codes(
						false,
					)
					.with_stop_on_first_fatal_error(false)
					.with_enable_error_reporting_gpio(
						false,
					), // FIXME add values (which have fine defaults) eventually: enable_error_reporting, enable_error_reporting_gpio, enable_error_reporting_beep_codes, enable_using_handshake, input_port: 132.into(), input_port, output_delay, output_port
				// FIXME: stop_on_first_fatal_error: false.into(), input_port_size: 4.into(), output_port_size: 4.into(), input_port_type: 6.into(), output_port_type: 6.into(), clear_acknowledgement: false.into(), error_reporting_gpio: Gpio { pin: 85, iomux_control: 1, bank_control: 192 }, enable_heart_beat: false.into() }
				&[],
			)?;
		}
		_ => {
			todo!();
		}
	}

	apcb.insert_struct_entry::<ExtVoltageControl>(
		EntryId::Memory(MemoryEntryId::ExtVoltageControl),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&ExtVoltageControl::new_enabled(
			PortType::FchHtIo,
			0x84,
			PortSize::_32Bit,
			PortType::FchHtIo,
			0x80,
			PortSize::_32Bit,
			false,
		),
		&[],
	)?;

	apcb.insert_struct_sequence_as_entry(
		EntryId::Memory(MemoryEntryId::PlatformTuning),
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
		&[&amd_apcb::memory::platform_tuning::Terminator::new()],
	)?;

	// Note: apcb.insert_entry is done implicity

	let mut tokens = apcb.tokens_mut(
		0,
		BoardInstances::all(),
		PriorityLevels::from_level(PriorityLevel::Normal),
	)?;

	tokens.set_psp_measure_config(0x0)?;
	tokens.set_psp_enable_debug_mode(PspEnableDebugMode::Disabled)?; // Byte
	tokens.set_psp_tp_port(true)?;
	tokens.set_psp_event_log_display(true)?;
	tokens.set_psp_psb_auto_fuse(true)?;
	tokens.set_psp_error_display(true)?;
	tokens.set_psp_stop_on_error(false)?;
	tokens.set_psp_syshub_watchdog_timer_interval(0xa28)?; // Word

	tokens.set_abl_serial_baud_rate(BaudRate::_115200Baud)?; // Byte

	tokens.set_pmu_training_mode(MemControllerPmuTrainingMode::_1D_2D)?; // OBSOLETE 24
	tokens.set_mem_training_hdt_control(
		MemTrainingHdtControl::StageCompletionMessages1,
	)?; // Byte ; FIXME: +1
	tokens.set_display_pmu_training_results(false)?;
	tokens.set_performance_tracing(false)?;
	tokens.set_ecc_symbol_size(EccSymbolSize::x16)?; // OBSOLETE 30
	tokens.set_cpu_fetch_from_spi_ap_base(0xfff00000)?; // DWord
	tokens.set_vga_program(true)?;

	tokens.set_fch_console_out_mode(0)?;
	tokens.set_fch_smbus_speed(FchSmbusSpeed::Value(0x2a))?; // Byte; x in 66 MHz / (4 x); FIXME: Auto?!
	tokens.set_fch_console_out_super_io_type(
		FchConsoleOutSuperIoType::Auto,
	)?; // Byte
	tokens.set_fch_console_out_basic_enable(0x0)?; // Byte // OBSOLETE 21
	tokens.set_fch_console_out_serial_port(
		FchConsoleSerialPort::Uart0Mmio,
	)?; // Byte
	tokens.set_fch_gpp_clk_map(FchGppClkMap::Auto)?; // Word
	tokens.set_fch_rom3_base_high(0x0)?; // DWord

	tokens.set_ccx_min_sev_asid(0x1)?; // DWord
	tokens.set_ccx_ppin_opt_in(false)?;
	tokens.set_ccx_sev_asid_count(CcxSevAsidCount::_509)?; // Byte

	tokens.set_bmc_init_before_dram(false)?;
	tokens.set_bmc_link_speed(BmcLinkSpeed::PcieGen1)?;
	tokens.set_bmc_start_lane(0x81)?; // Byte // OBSOLETE 23
	tokens.set_bmc_end_lane(0x81)?; // Byte // OBSOLETE 9
	tokens.set_bmc_socket(0x0)?; // Byte // OBSOLETE 19
	tokens.set_bmc_device(0x5)?; // Byte // OBSOLETE 25
	tokens.set_bmc_function(0x2)?; // Byte // OBSOLETE 11
	tokens.set_configure_second_pcie_link(false)?;
	tokens.set_second_pcie_link_max_payload(
		SecondPcieLinkMaxPayload::HardwareDefault,
	)?; // Byte
	tokens.set_second_pcie_link_speed(SecondPcieLinkSpeed::Gen2)?; // Byte

	tokens.set_mem_quad_rank_capable(true)?; // OBSOLETE 6
	tokens.set_mem_sodimm_capable(true)?;
	tokens.set_mem_rdimm_capable(true)?;
	tokens.set_mem_lrdimm_capable(true)?; // ? // leaving this off is bad
	tokens.set_mem_mode_unganged(true)?; // ? // leaving this off is bad
	tokens.set_mem_dimm_type_ddr3_capable(false)?; // ? // leaving this off is bad
	tokens.set_mem_dimm_type_lpddr3_capable(false)?;
	tokens.set_mem_force_power_down_throttle_enable(false)?;
	tokens.set_mem_dqs_training_control(true)?;
	tokens.set_mem_enable_parity(true)?;
	tokens.set_mem_udimm_capable(true)?;
	tokens.set_mem_enable_bank_group_swap(true)?;
	tokens.set_mem_channel_interleaving(false)?;
	tokens.set_mem_pstate(true)?;
	tokens.set_mem_limit_memory_to_below_1_TiB(true)?;
	tokens.set_mem_enable_bank_swizzle(false)?;
	tokens.set_mem_spd_read_optimization_ddr4(true)?;
	tokens.set_mem_hole_remapping(true)?;
	tokens.set_mem_oc_vddio_control(false)?;
	tokens.set_mem_enable_chip_select_interleaving(false)?;
	tokens.set_mem_uma_above_4_GiB(true)?;
	tokens.set_mem_ignore_spd_checksum(true)?;
	tokens.set_mem_ecc_sync_flood(false)?;
	tokens.set_u0x8f84dcb4(false)?; // Bool
	tokens.set_mem_nvdimm_n_disable(false)?;
	tokens.set_u0x96176308(true)?; // Bool
	tokens.set_mem_dram_double_refresh_rate(0x0)?; // Byte
					       // TODO: Try to remove and boot
	tokens.set_mem_dram_double_refresh_rate_unused(false)?; // Bool
	tokens.set_mem_sw_cmd_throttle_enable(false)?;
	tokens.set_mem_enable_bank_group_swap_alt(true)?;
	tokens.set_mem_on_die_thermal_sensor(true)?;
	tokens.set_mem_all_clocks(true)?;
	tokens.set_mem_enable_power_down(true)?;
	tokens.set_mem_uncorrected_ecc_retry_ddr4(true)?;
	tokens.set_cbs_mem_uncorrected_ecc_retry_ddr4(true)?;
	tokens.set_mem_odts_cmd_throttle_enable(true)?;
	tokens.set_mem_clear(false)?;
	tokens.set_mem_post_package_repair_enable(true)?;
	tokens.set_mem_ddr4_force_data_mask_disable(false)?;
	tokens.set_mem_enable_ecc_feature(true)?;
	tokens.set_mem_ecc_redirection(false)?;
	tokens.set_mem_ddr_route_balanced_tee(false)?;
	tokens.set_mem_temp_controlled_refresh_enable(false)?;
	tokens.set_mem_temp_controlled_extended_refresh(false)?; // OBSOLETE 7
	tokens.set_mem_restore_control(false)?;
	tokens.set_mem_override_dimm_spd_max_activity_count(
		MemMaxActivityCount::Auto,
	)?; // Byte
	tokens.set_mem_urg_ref_limit(0x6)?; // Byte
	tokens.set_u0x190305df(0x0)?; // Byte // OBSOLETE 10
	tokens.set_uma_mode(UmaMode::Auto)?; // OBSOLETE 12
	tokens.set_workload_profile(WorkloadProfile::Disabled)?; // Byte
	tokens.set_mem_nvdimm_power_source(
		MemNvdimmPowerSource::DeviceManaged,
	)?; // OBSOLETE 13
	tokens.set_mem_dram_address_command_parity_retry_count(0x1)?; // Byte
	tokens.set_mem_data_poison(MemDataPoison::Enabled)?; // Byte // OBSOLETE 14
	tokens.set_mem_roll_window_depth(
		MemThrottleCtrlRollWindowDepth::Memclks(
			NonZeroU8::new(0xff)
				.ok_or(amd_apcb::Error::TokenRange)?,
		),
	)?; // Byte
	tokens.set_mem_heal_ppr_type(MemHealPprType::SoftRepair)?; // Byte
	tokens.set_mem_heal_test_select(MemHealTestSelect::Normal)?; // Byte
	tokens.set_mem_heal_max_bank_fails(0x3)?; // Byte
	tokens.set_mem_heal_bist_enable(MemHealBistEnable::Disabled)?; // Byte
	tokens.set_mem_rcd_parity(true)?; // Byte
	tokens.set_odts_cmd_throttle_cycles(0x57)?; // Byte // OBSOLETE 15
	tokens.set_u0x6c4ccf38(0x0)?; // Byte // OBSOLETE 16

	tokens.set_mem_data_scramble(0x1)?; // Byte // OBSOLETE 20
	tokens.set_mem_dram_vref_range(0x0)?; // OBSOLETE 22
	tokens.set_mem_cpu_vref_range(0x0)?; // Byte // OBSOLETE 17
	tokens.set_u0xae7f0df4(0xff)?; // Byte

	tokens.set_df_group_d_platform(true)?;
	tokens.set_df_bottom_io(0xb0)?; // Byte
	tokens.set_df_pci_mmio_size(0x10000000)?; // DWord
	tokens.set_df_remap_at_1tib(DfRemapAt1TiB::Auto)?; // Byte
	tokens.set_df_invert_dram_map(DfToggle::Auto)?; // Byte
	tokens.set_df_mem_interleaving(DfMemInterleaving::Auto)?; // Byte
	tokens.set_df_mem_interleaving_size(DfMemInterleavingSize::Auto)?; // Byte
	tokens.set_df_gmi_encrypt(DfToggle::Auto)?; // Byte
	tokens.set_df_probe_filter(DfToggle::Auto)?; // Byte
	tokens.set_df_xgmi_encrypt(DfToggle::Auto)?; // Byte
	tokens.set_df_dram_numa_per_socket(DfDramNumaPerSocket::Auto)?; // Byte
	tokens.set_df_4link_max_xgmi_speed(0xff)?; // Byte
	tokens.set_df_3link_max_xgmi_speed(0xff)?; // Byte
	tokens.set_df_save_restore_mem_encrypt(DfToggle::Auto)?; // Byte
	tokens.set_df_mem_clear(DfToggle::Auto)?;
	tokens.set_df_xgmi_tx_eq_mode(DfXgmiTxEqMode::Auto)?; // Byte
	tokens.set_df_pstate_mode_select(DfPstateModeSelect::Auto)?;
	tokens.set_df_cake_crc_threshold_bounds(
		DfCakeCrcThresholdBounds::Value(0x64),
	)?; // DWord; Percentage is 0.00001% * x
	tokens.set_df_xgmi_config(DfXgmiLinkConfig::Auto)?; // Byte

	tokens.set_pcie_reset_control(true)?; // OBSOLETE 8
	tokens.set_pcie_reset_gpio_pin(0xffffffff)?; // DWord
	tokens.set_pcie_reset_pin_select(0x2)?; // Byte

	tokens.set_mem_user_timing_mode(MemUserTimingMode::Auto)?; // DWord
	tokens.set_mem_self_refresh_exit_staggering(
		MemSelfRefreshExitStaggering::Disabled,
	)?; // Byte
	tokens.set_mem_controller_writing_crc_mode(
		MemControllerWritingCrcMode::Disabled,
	)?;
	tokens.set_mem_controller_writing_crc_max_replay(0x8)?; // Byte
	tokens.set_mem_controller_writing_crc_limit(0x0)?; // Byte
	tokens.set_mem_parity_error_max_replay_ddr4(8)?; // Byte
	tokens.set_mem_rdimm_timing_rcd_f0rc0f_additional_latency(
		MemRdimmTimingCmdParLatency::Auto,
	)?;
	tokens.set_sw_cmd_throt_cycles(0x0)?; // OBSOLETE 26
	tokens.set_mem_sub_urg_ref_lower_bound(0x4)?; // Byte

	tokens.set_dimm_sensor_resolution(0x1)?; // OBSOLETE 18
	tokens.set_dimm_sensor_lower(0xa)?; // OBSOLETE 38
	tokens.set_dimm_sensor_upper(0x50)?; // OBSOLETE 36
	tokens.set_dimm_sensor_critical(0x5f)?; // OBSOLETE 31
	tokens.set_dimm_sensor_config(0x408)?; // OBSOLETE 32
	tokens.set_dimm_3ds_sensor_critical(0x50)?; // Word // OBSOLETE 27; Milan
	tokens.set_dimm_3ds_sensor_upper(0x42)?; // Word // OBSOLETE 29; Milan

	tokens.set_scrub_icache_rate(0x0)?; // OBSOLETE 33
	tokens.set_scrub_dram_rate(0x0)?; // OBSOLETE 34
	tokens.set_scrub_dcache_rate(0x0)?; // OBSOLETE 35
	tokens.set_scrub_l2_rate(0x0)?; // OBSOLETE 28
	tokens.set_scrub_l3_rate(0x0)?; // OBSOLETE 37

	tokens.set_mem_bus_frequency_limit(MemClockValue::Ddr3200)?; // DWord

	tokens.set_mem_power_down_mode(0x0)?; // DWord
	tokens.set_mem_uma_size(0x0)?; // DWord
	tokens.set_mem_uma_alignment(0xffffc0)?; // DWord

	tokens.set_mem_clock_value(MemClockValue::Ddr2400)?; // DWord

	tokens.set_mem_action_on_bist_failure(
		MemActionOnBistFailure::DoNothing,
	)?; // Byte
	tokens.set_mem_mbist_aggressor_static_lane_control(false)?;
	tokens.set_mem_mbist_tgt_static_lane_control(false)?;
	tokens.set_mem_mbist_aggressor_on(false)?; // Obsolete
	tokens.set_mem_mbist_worse_cas_granularity(0x0)?; // Byte
	tokens.set_mem_mbist_read_data_eye_voltage_step(0x1)?; // Byte
	tokens.set_mem_mbist_data_eye_silent_execution(false)?; // Byte
	tokens.set_mem_mbist_aggressor_static_lane_val(0x0)?; // Byte
	tokens.set_mem_mbist_tgt_static_lane_val(0x0)?; // Byte
	tokens.set_mem_mbist_data_eye_type(MemMbistDataEyeType::_1dTiming)?; // Byte
	tokens.set_mem_mbist_test_mode(MemMbistTestMode::PhysicalInterface)?; // Byte // MBIST AND OBSOLETE 1
	tokens.set_mem_mbist_aggressor_static_lane_sel_ecc(0x0)?; // Byte
	tokens.set_mem_mbist_read_data_eye_timing_step(0x1)?; // Byte
	tokens.set_mem_mbist_data_eye_execution_repeat_count(0x1)?; // Byte // MBIST AND OBSOLETE 2
	tokens.set_mem_mbist_tgt_static_lane_sel_ecc(0x0)?; // Byte // MBIST AND OBSOLETE 3
	tokens.set_mem_mbist_pattern_length(0x3)?; // Byte
	tokens.set_mem_mbist_halt_on_error(0x1)?; // Byte // MBIST AND OBSOLETE 4
	tokens.set_mem_mbist_write_data_eye_voltage_step(0x1)?; // Byte
	tokens.set_mem_mbist_per_bit_slave_die_report(0x0)?; // Byte
	tokens.set_mem_mbist_write_data_eye_timing_step(0x1)?; // Byte
	tokens.set_mem_mbist_aggressors_channels(
		MemMbistAggressorsChannels::Disabled,
	)?; // Byte
	tokens.set_mem_mbist_test(MemMbistTest::Disabled)?; // Byte // MBIST AND OBSOLETE 5
	tokens.set_mem_mbist_pattern_select(MemMbistPatternSelect::Prbs)?; // Byte
	tokens.set_mem_mbist_aggressor_static_lane_sel_lo(0x0)?; // DWord
	tokens.set_mem_mbist_aggressor_static_lane_sel_hi(0x0)?; // DWord
	tokens.set_mem_mbist_tgt_static_lane_sel_lo(0x0)?; // DWord
	tokens.set_mem_mbist_tgt_static_lane_sel_hi(0x0)?; // DWord

	tokens.set_mem_self_heal_bist_timeout(0x2710)?; // DWord

	tokens.set_dxio_vga_api_enable(false)?;
	tokens.set_dxio_phy_param_iqofc(DxioPhyParamIqofc::Value(0x7fffffff))?; // DWord
	tokens.set_dxio_phy_param_pole(DxioPhyParamPole::Skip)?; // DWord
	tokens.set_dxio_phy_param_vga(DxioPhyParamVga::Skip)?; // DWord
	tokens.set_dxio_phy_param_dc(DxioPhyParamDc::Skip)?; // DWord

	match processor_generation {
		ProcessorGeneration::Naples => {
			panic!("not supported");
		}
		ProcessorGeneration::Rome => {
			tokens.set_mother_board_type_0(false)?;
			tokens.set_mctp_reroute_enable(false)?;
			tokens.set_iohc_mixed_rw_workaround(false)?;
			tokens.set_df_sys_storage_at_top_of_mem(3)?; // FIXME: 0: distributed, 1: consolidated; 0xff: auto
			tokens.set_u0x28eb57ad(0x1e)?; // 0x0E~0x3E; XXX
			tokens.set_bmc_vga_io_enable(false)?;
			tokens.set_bmc_vga_io_port(0)?;
			tokens.set_bmc_vga_io_port_size(0)?;
			tokens.set_bmc_vga_io_bar_to_replace(0)?;
			tokens.set_bmc_gen2_tx_deemphasis(
				BmcGen2TxDeemphasis::Disabled,
			)?;
			tokens.set_mem_tsme_mode_rome(MemTsmeMode::Disabled)?;
		}
		ProcessorGeneration::Milan => {
			tokens.set_gnb_additional_features(true)?; // [optional]
			tokens.set_gnb_additional_feature_dsm(true)?;
			tokens.set_mem_amp(true)?;
			tokens.set_gnb_additional_feature_l3_performance_bias(
				true,
			)?;
			tokens.set_gnb_additional_feature_dsm_detector(true)?;
			tokens.set_gnb_smu_df_pstate_fclk_limit(
				GnbSmuDfPstateFclkLimit::Auto,
			)?;
			tokens.set_gnb_off_ramp_stall(0xc8)?; // DWord // ?
			tokens.set_mem_tsme_mode_milan(false)?;
		}
		_ => {
			todo!();
		}
	}

	Apcb::update_checksum(&mut buf)?;
	let mut xbuf = &buf[..]; // TODO: cut off at APCB_SIZE
	let size = xbuf.len();
	bhd_directory
		.add_blob_entry(
			None,
			attrs,
			size.try_into().unwrap(),
			None,
			&mut |buf: &mut [u8]| {
				let bytes = if xbuf.len() > buf.len() {
					buf.len()
				} else {
					xbuf.len()
				};
				let (a, b) = xbuf.split_at(bytes);
				(&mut buf[.. a.len()]).copy_from_slice(a);
				xbuf = b;
				Ok(bytes)
			},
		)
		.unwrap();

	Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(
	name = "amd-host-image-builder",
	about = "Build host flash image for AMD Zen CPUs."
)]
struct Opts {
	#[structopt(short = "g", long = "generation")]
	host_processor_generation: ProcessorGeneration,

	#[structopt(short = "o", long = "output-file", parse(from_os_str))]
	output_filename: PathBuf,

	#[structopt(short = "r", long = "reset-image", parse(from_os_str))]
	reset_image_filename: PathBuf,
}

fn main() -> std::io::Result<()> {
	//let args: Vec<String> = env::args().collect();
	let opts = Opts::from_args();

	let filename = opts.output_filename;
	let file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.open(filename)?;
	file.set_len(IMAGE_SIZE.into())?;
	let mut storage = FlashImage::new(file);
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
	let host_processor_generation = opts.host_processor_generation;
	let mut efs = match Efs::<_, ERASABLE_BLOCK_SIZE>::create(
		storage,
		host_processor_generation,
	) {
		Ok(efs) => efs,
		Err(e) => {
			eprintln!("Error on creation: {:?}", e);
			std::process::exit(1);
		}
	};
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
		_ => todo!(),
	};
	let mut psp_directory = efs
		.create_psp_directory(
			AlignedLocation::try_from(0x12_0000).unwrap(),
			AlignedLocation::try_from(0x24_0000).unwrap(),
		)
		.unwrap();
	psp_entry_add_from_file(
		&mut psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::AmdPublicKey),
		firmware_blob_directory_name.join("AmdPubKey.tkn"),
	)
	.unwrap();
	psp_entry_add_from_file(
		&mut psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::PspBootloader),
		firmware_blob_directory_name.join("PspBootLoader.sbin"),
	)
	.unwrap();
	psp_entry_add_from_file(
		&mut psp_directory,
		None,
		&PspDirectoryEntryAttrs::new().with_type_(
			PspDirectoryEntryType::PspRecoveryBootloader,
		),
		firmware_blob_directory_name.join("PspRecoveryBootLoader.sbin"),
	)
	.unwrap();
	psp_entry_add_from_file(
		&mut psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::SmuOffChipFirmware8),
		firmware_blob_directory_name.join("SmuFirmware.csbin"),
	)
	.unwrap();
	if host_processor_generation != ProcessorGeneration::Rome {
		// Note: Cannot remove this entry (otherwise postcode 0xE022 error).
		psp_entry_add_from_file(
			&mut psp_directory,
			None,
			&PspDirectoryEntryAttrs::new().with_type_(
				PspDirectoryEntryType::AmdSecureDebugKey,
			),
			firmware_blob_directory_name
				.join("SecureDebugToken.stkn"),
		)
		.unwrap();
	}
	psp_directory_add_default_entries(
		&mut psp_directory,
		&firmware_blob_directory_name,
	)
	.unwrap();
	psp_entry_add_from_file(
		&mut psp_directory,
		None,
		&PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::DxioPhySramFirmware),
		firmware_blob_directory_name.join("PhyFw.sbin"),
	)
	.unwrap();

	if host_processor_generation == ProcessorGeneration::Rome {
		psp_entry_add_from_file(
			&mut psp_directory,
			None,
			&PspDirectoryEntryAttrs::new().with_type_(
				PspDirectoryEntryType::DxioPhySramPublicKey,
			),
			firmware_blob_directory_name.join("PhyFwSb4kr.stkn"),
		)
		.unwrap();
		psp_entry_add_from_file(
			&mut psp_directory,
			None,
			&PspDirectoryEntryAttrs::new().with_type_(
				PspDirectoryEntryType::PmuPublicKey,
			),
			firmware_blob_directory_name
				.join("Starship-PMU-FW.stkn"),
		)
		.unwrap();
	} else {
		/* optional psp_entry_add_from_file(
		    &mut psp_directory,
		    None,
		    &PspDirectoryEntryAttrs::new()
			.with_type_(PspDirectoryEntryType::DrtmTa),
		    firmware_blob_directory_name.join("PSP-DRTM.sbin"),
		)
		.unwrap(); */
		psp_entry_add_from_file(
            &mut psp_directory,
            None,
            &PspDirectoryEntryAttrs::new()
                .with_type_(PspDirectoryEntryType::PspBootloaderPublicKeysTable),
            firmware_blob_directory_name.join("PSP-Key-DB.sbin"),
        )
        .unwrap();
	}

	//    let mut second_level_psp_directory = efs.create_second_level_psp_directory(AlignedLocation::try_from(0x2c_0000).unwrap(), AlignedLocation::try_from(0x2c_0000 + 0x12_0000).unwrap()).unwrap();
	//
	//    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspBootloader),
	//        firmware_blob_directory_name.join("PspBootLoader.sbin"),
	//    ).unwrap();
	//    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SmuOffChipFirmware8),
	//        firmware_blob_directory_name.join("SmuFirmware.csbin"),
	//    ).unwrap();
	//    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AmdSecureDebugKey),
	//        firmware_blob_directory_name.join("SecureDebugToken.stkn"),
	//    ).unwrap(); // XXX cannot remove
	//    psp_directory_add_default_entries(&mut second_level_psp_directory, &firmware_blob_directory_name).unwrap();
	//
	// /* removed    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SevData),
	//        firmware_blob_directory_name.join("SevData.unsorted"),
	//    ).unwrap();
	//
	//    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SevCode),
	//        firmware_blob_directory_name.join("SevCode.unsorted"),
	//    ).unwrap();*/
	//
	//    psp_entry_add_from_file(
	//        &mut second_level_psp_directory,
	//        None,
	//        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::DxioPhySramFirmware),
	//        firmware_blob_directory_name.join("PhyFw.sbin"),
	//    ).unwrap();
	//
	//    if host_processor_generation == ProcessorGeneration::Milan {
	//        psp_entry_add_from_file(
	//            &mut second_level_psp_directory,
	//            None,
	//            &PspDirectoryEntryAttrs::new()
	//                .with_type_(PspDirectoryEntryType::PspBootloaderPublicKeysTable),
	//            firmware_blob_directory_name.join("PSP-Key-DB.sbin"),
	//        )
	//        .unwrap();
	//    }

	let mut bhd_directory = efs
		.create_bhd_directory(
			AlignedLocation::try_from(0x24_0000).unwrap(),
			AlignedLocation::try_from(0x24_0000 + 0x8_0000)
				.unwrap(),
		)
		.unwrap();

	bhd_add_apcb(
        host_processor_generation,
        &mut bhd_directory,
        &match host_processor_generation {
            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new()
                .with_type_(BhdDirectoryEntryType::ApcbBackup)
                .with_sub_program(1),
            ProcessorGeneration::Rome => {
                BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup)
            }
            ProcessorGeneration::Naples => {
                BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup)
            }
            _ => {
                todo!();
            }
        },
    );

	bhd_directory
		.add_apob_entry(None, BhdDirectoryEntryType::Apob, 0x400_0000)
		.unwrap();

	bhd_directory_add_reset_image(
		&mut bhd_directory,
		&opts.reset_image_filename,
	)
	.unwrap();
	bhd_directory_add_default_entries(
		&mut bhd_directory,
		&firmware_blob_directory_name,
	)
	.unwrap();

	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(8)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Udimm_Imem.csbin"),
		None,
	)
	.unwrap();
	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(8)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Udimm_Dmem.csbin"),
		None,
	)
	.unwrap();

	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(9)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Rdimm_Imem.csbin"),
		None,
	)
	.unwrap();
	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(9)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Rdimm_Dmem.csbin"),
		None,
	)
	.unwrap();

	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(
				BhdDirectoryEntryType::PmuFirmwareInstructions,
			)
			.with_instance(8)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Udimm_Imem.csbin"),
		None,
	)
	.unwrap();
	bhd_entry_add_from_file_if_present(
		&mut bhd_directory,
		None,
		&BhdDirectoryEntryAttrs::new()
			.with_type_(BhdDirectoryEntryType::PmuFirmwareData)
			.with_instance(8)
			.with_sub_program(1),
		firmware_blob_directory_name
			.join("Appb_BIST_Ddr4_Udimm_Dmem.csbin"),
		None,
	)
	.unwrap();
	//    let firmware_blob_directory_name = Path::new("amd-firmware/MILAN-b").join("second-bhd");
	//    let mut secondary_bhd_directory = bhd_directory.create_subdirectory(AlignedLocation::try_from(0x3e_0000).unwrap(), AlignedLocation::try_from(0x3e_0000 + 0x8_0000).unwrap()).unwrap();
	//
	//    // FIXME: if Milan
	//
	//    bhd_entry_add_from_file_with_custom_size(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &match host_processor_generation {
	//            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup).with_sub_program(1),
	//            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
	//        },
	//        Apcb::MAX_SIZE,
	//        apcb_source_file_name.as_path(),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file_with_custom_size(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &match host_processor_generation {
	//            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup).with_instance(8).with_sub_program(1),
	//            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
	//        },
	//        544,
	//        Path::new("amd-firmware/MILAN-b/second-bhd/ApcbBackup_8.unsorted"),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file_with_custom_size(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &match host_processor_generation {
	//            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup).with_instance(9).with_sub_program(1),
	//            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
	//        },
	//        672,
	//        Path::new("amd-firmware/MILAN-b/second-bhd/ApcbBackup_9.unsorted"),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file_with_custom_size(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &match host_processor_generation {
	//            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::Apcb).with_instance(0).with_sub_program(1),
	//            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::Apcb),
	//        },
	//        4096,
	//        Path::new("amd-firmware/MILAN-b/second-bhd/Apcb.unsorted"),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file_with_custom_size(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &match host_processor_generation {
	//            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::Apcb).with_instance(1).with_sub_program(1),
	//            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::Apcb),
	//        },
	//        4096,
	//        Path::new("amd-firmware/MILAN-b/second-bhd/Apcb_1.unsorted"),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_directory_add_reset_image(&mut secondary_bhd_directory, &opts.reset_image_filename).unwrap();
	//    bhd_directory_add_default_entries(&mut secondary_bhd_directory, &firmware_blob_directory_name).unwrap();
	//
	//    bhd_entry_add_from_file(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &BhdDirectoryEntryAttrs::new()
	//            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
	//            .with_instance(0),
	//        Path::new("amd-firmware/MILAN-b/second-bhd/MicrocodePatch.unsorted").to_path_buf(),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &BhdDirectoryEntryAttrs::new()
	//            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
	//            .with_instance(1),
	//        Path::new("amd-firmware/MILAN-b/second-bhd/MicrocodePatch_1.unsorted").to_path_buf(),
	//        None,
	//    )
	//    .unwrap();
	//
	//    bhd_entry_add_from_file(
	//        &mut secondary_bhd_directory,
	//        None,
	//        &BhdDirectoryEntryAttrs::new()
	//            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
	//            .with_instance(2),
	//        Path::new("amd-firmware/MILAN-b/second-bhd/MicrocodePatch_2.unsorted").to_path_buf(),
	//        None,
	//    )
	//    .unwrap();

	//            println!("{:?}", efh);
	Ok(())
}
