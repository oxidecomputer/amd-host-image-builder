use amd_efs::{
    BhdDirectory, BhdDirectoryEntryAttrs, BhdDirectoryEntryType, Efs, ProcessorGeneration,
    PspDirectory, PspDirectoryEntryAttrs, PspDirectoryEntryType,
    PspSoftFuseChain
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

use amd_apcb::Apcb;
//use amd_efs::ProcessorGeneration;
use amd_flash::{FlashRead, FlashWrite, Location, ErasableLocation};

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

impl<const ERASABLE_BLOCK_SIZE: usize> FlashRead<ERASABLE_BLOCK_SIZE> for FlashImage {
    fn read_exact(&self, location: Location, buffer: &mut [u8]) -> amd_flash::Result<usize> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(amd_flash::Error::Io);
            }
        }
        match file.read_exact(buffer) {
            Ok(()) => {
                Ok(buffer.len())
            }
            Err(e) => {
                return Err(amd_flash::Error::Io);
            }
        }
    }
    fn read_erasable_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>, buffer: &mut [u8; ERASABLE_BLOCK_SIZE]) -> amd_flash::Result<()> {
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

impl<const ERASABLE_BLOCK_SIZE: usize>
    FlashWrite<ERASABLE_BLOCK_SIZE> for FlashImage
{
    fn erase_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>) -> amd_flash::Result<()> {
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
    fn erase_and_write_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>, buffer: &[u8; ERASABLE_BLOCK_SIZE]) -> amd_flash::Result<()> {
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
    directory.add_blob_entry(payload_position, attrs, size.try_into().unwrap(), &mut |buf: &mut [u8]| {
        let mut cursor = 0;
        loop {
            let bytes = source.read(&mut buf[cursor ..]).map_err(|_| {
                amd_efs::Error::Marshal
            })?;
            if bytes == 0 {
                return Ok(cursor);
            }
            cursor += bytes;
        }
    })?;
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
    ram_destination_address: Option<u64>
) -> amd_efs::Result<()>
where T: std::io::Read
{
    directory.add_blob_entry(payload_position, attrs, size.try_into().unwrap(), ram_destination_address, &mut |buf: &mut [u8]| {
        let mut cursor = 0;
        loop {
            let bytes = source.read(&mut buf[cursor ..]).map_err(|_| {
                amd_efs::Error::Marshal
            })?;
            if bytes == 0 {
                return Ok(cursor);
            }
            cursor += bytes;
        }
    })?;
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

    bhd_entry_add_from_reader_with_custom_size(directory, payload_position,
        attrs, size, &mut reader, ram_destination_address)
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
    bhd_entry_add_from_file_with_custom_size(directory, payload_position, attrs, size, &source_filename, ram_destination_address)
}

fn psp_directory_add_default_entries(psp_directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>, firmware_blob_directory_name: &PathBuf) -> amd_efs::Result<()> {
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AblPublicKey),
        firmware_blob_directory_name.join("AblPubKey.bin"), // that was weird: "PspABLFw_gn.stkn", // imm
    )?;

    psp_directory.add_value_entry(
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspSoftFuseChain),
        PspSoftFuseChain::new().with_secure_debug_unlock(true).into(),
    )?;

    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SmuOffChipFirmware12),
        firmware_blob_directory_name.join("SmuFirmware2.csbin"),
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new()
            .with_type_(PspDirectoryEntryType::PspEarlySecureUnlockDebugImage),
        firmware_blob_directory_name.join("SecureDebugUnlock.sbin"),
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::WrappedIkek),
        firmware_blob_directory_name.join("PspIkek.bin"), // imm
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspTokenUnlockData),
        firmware_blob_directory_name.join("SecureEmptyToken.bin"), // imm
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SecurityPolicyBinary),
        firmware_blob_directory_name.join("RsmuSecPolicy.sbin"),
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::Mp5Firmware),
        firmware_blob_directory_name.join("Mp5.csbin"),
    )?;
    psp_entry_add_from_file(
        psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::Abl0),
        firmware_blob_directory_name.join("AgesaBootloader_U_prod.csbin"),
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
        bhd_entry_add_from_file(directory, payload_position, attrs, source_filename, ram_destination_address)
    } else {
        Ok(())
    }
}

fn elf_symbol(binary: &goblin::elf::Elf, key: &str) -> Option<goblin::elf::Sym> {
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

fn bhd_directory_add_reset_image(bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>, reset_image_filename: &Path) -> Result<()> {
    let buffer = fs::read(reset_image_filename).map_err(|x| Error::Io(x))?;
    let mut destination_origin: Option<u64> = None;
    let mut iov = Box::new(std::io::empty()) as Box<dyn Read>;
    let sz;

    match goblin::Object::parse(&buffer).map_err(|_| Error::IncompatibleExecutable)? {
        goblin::Object::Elf(binary) => {
            let mut last_vaddr = 0u64;
            let mut holesz = 0usize;
            let mut totalsz = 0usize;
            if binary.header.e_type != goblin::elf::header::ET_EXEC
            || binary.header.e_machine != goblin::elf::header::EM_X86_64
            || binary.header.e_version < goblin::elf::header::EV_CURRENT.into() {
                return Err(Error::IncompatibleExecutable)
            }
            for header in &binary.program_headers {
                if header.p_type == goblin::elf::program_header::PT_LOAD {
                    eprintln!("PROG {:x?}", header);
                    if header.p_memsz == 0 {
                        continue;
                    }
                    if destination_origin == None {
                        // Note: File is sorted by p_vaddr.
                        destination_origin = Some(header.p_vaddr);
                        last_vaddr = header.p_vaddr;
                    }
                    if header.p_vaddr < last_vaddr {
                        // According to ELF standard, this should not happen
                        return Err(Error::IncompatibleExecutable)
                    }
                    if header.p_filesz > header.p_memsz {
                        // According to ELF standard, this should not happen
                        return Err(Error::IncompatibleExecutable)
                    }
                    if header.p_paddr != header.p_vaddr {
                        return Err(Error::IncompatibleExecutable)
                    }
                    if header.p_filesz > 0 {
                        if header.p_vaddr > last_vaddr {
                            holesz += (header.p_vaddr - last_vaddr) as usize;
                        }
                        if holesz > 0 {
                            eprintln!("hole: {:x}", holesz);
                            iov = Box::new(iov.chain(Hole::new(holesz))) as Box<dyn Read>;
                            totalsz += holesz;
                            holesz = 0;
                        }
                        let chunk = &buffer[header.p_offset as usize ..
                            (header.p_offset + header.p_filesz) as usize];
                        eprintln!("chunk: {:x} @ {:x}", header.p_filesz, header.p_offset);
                        iov = Box::new(iov.chain(chunk)) as Box<dyn Read>;
                        totalsz += header.p_filesz as usize;
                        if header.p_memsz > header.p_filesz {
                            holesz += (header.p_memsz - header.p_filesz) as usize;
                        }
                        last_vaddr = header.p_vaddr + header.p_memsz;
                    }
                }
            }
            for header in &binary.section_headers {
                eprintln!("SECTION {:x?}", header);
            }
            if let Some(mut iter) = binary.iter_note_headers(&buffer) {
                while let Some(Ok(a)) = iter.next() {
                    eprintln!("NOTE HEADER {:x?}", a);
                }
            }
            if let Some(mut iter) = binary.iter_note_sections(&buffer, None) {
                while let Some(Ok(a)) = iter.next() {
                    eprintln!("NOTE SECTION {:x?}", a);
                }
            }
            // SYMBOL "_BL_SPACE" Sym { st_name: 5342, st_info: 0x0 LOCAL NOTYPE, st_other: 0 DEFAULT, st_shndx: 65521, st_value: 0x29000, st_size: 0 }
            // The part of the program we copy into the flash image should be
            // of the same size as the space allocated at loader build time.
            let symsz = elf_symbol(&binary, "_BL_SPACE").ok_or(Error::IncompatibleExecutable)?.st_value;
            eprintln!("_BL_SPACE: {:x?}", symsz);
            if totalsz != symsz as usize {
                return Err(Error::IncompatibleExecutable)
            }
            sz = totalsz;

            // These symbols have been embedded into the loader to serve as
            // checks in this exact application.
            let sloader = elf_symbol(&binary, "__sloader").ok_or(Error::IncompatibleExecutable)?.st_value;
            eprintln!("__sloader: {:x?}", sloader);
            if sloader != destination_origin.ok_or(Error::IncompatibleExecutable)? {
                return Err(Error::IncompatibleExecutable)
            }

            let eloader = elf_symbol(&binary, "__eloader").ok_or(Error::IncompatibleExecutable)?.st_value;
            eprintln!("__eloader: {:x?}", eloader);
            if eloader != last_vaddr {
                return Err(Error::IncompatibleExecutable)
            }

            // The entry point (reset vector) must be 0x10 bytes below the
            // end of a (real-mode) segment--and that segment must begin at the end
            // of the loaded program.  See AMD pub 55758 sec. 4.3 item 4.
            if binary.header.e_entry != last_vaddr.checked_sub(0x10).ok_or(Error::IncompatibleExecutable)? {
                return Err(Error::IncompatibleExecutable)
            }
            if last_vaddr & 0xffff != 0 {
                return Err(Error::IncompatibleExecutable)
            }
        },
        _ => {
            destination_origin = Some(0x8000_0000u64.checked_sub(buffer.len() as u64).ok_or(Error::ImageTooBig)?);
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

fn bhd_directory_add_default_entries(bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>, firmware_blob_directory_name: &PathBuf) -> amd_efs::Result<()> {
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(1)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Udimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(1)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Udimm_Dmem.csbin"),
        None,
    )?;

    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(2)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Rdimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(2)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Rdimm_Dmem.csbin"),
        None,
    )?;

    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(3)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Lrdimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(3)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_1D_Ddr4_Lrdimm_Dmem.csbin"),
        None,
    )?;

    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(4)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Udimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(4)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Udimm_Dmem.csbin"),
        None,
    )?;

    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(5)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Rdimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(5)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Rdimm_Dmem.csbin"),
        None,
    )?;

    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(6)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Lrdimm_Imem.csbin"),
        None,
    )?;
    bhd_entry_add_from_file(
        bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(6)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_2D_Ddr4_Lrdimm_Dmem.csbin"),
        None,
    )?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "amd-host-image-builder", about = "Build host flash image for AMD Zen CPUs.")]
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
        FlashWrite::<ERASABLE_BLOCK_SIZE>::erase_block(&mut storage, position)
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
        ProcessorGeneration::Milan => Path::new("amd-firmware").join("milan"),
        ProcessorGeneration::Rome => Path::new("amd-firmware").join("rome"),
        ProcessorGeneration::Naples => Path::new("amd-firmware").join("naples"),
    };
    let mut psp_directory = efs.create_psp_directory(AlignedLocation::try_from(0x12_0000).unwrap(), AlignedLocation::try_from(0x24_0000).unwrap()).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AmdPublicKey),
        firmware_blob_directory_name.join("AmdPubKey.tkn"),
    ).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspBootloader),
        firmware_blob_directory_name.join("PspBootLoader.sbin"),
    ).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspRecoveryBootloader),
        firmware_blob_directory_name.join("PspRecoveryBootLoader.sbin"),
    ).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SmuOffChipFirmware8),
        firmware_blob_directory_name.join("SmuFirmware.csbin"),
    ).unwrap();
    if host_processor_generation != ProcessorGeneration::Rome {
        // Note: Cannot remove this entry (otherwise postcode 0xE022 error).
        psp_entry_add_from_file(
            &mut psp_directory,
            None,
            &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AmdSecureDebugKey),
            firmware_blob_directory_name.join("SecureDebugToken.stkn"),
        ).unwrap();
    }
    psp_directory_add_default_entries(&mut psp_directory, &firmware_blob_directory_name).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::DxioPhySramFirmware),
        firmware_blob_directory_name.join("PhyFw.sbin"),
    ).unwrap();

    if host_processor_generation == ProcessorGeneration::Rome {
        psp_entry_add_from_file(
            &mut psp_directory,
            None,
            &PspDirectoryEntryAttrs::new()
                .with_type_(PspDirectoryEntryType::DxioPhySramPublicKey),
            firmware_blob_directory_name.join("PhyFwSb4kr.stkn"),
        )
        .unwrap();
        psp_entry_add_from_file(
            &mut psp_directory,
            None,
            &PspDirectoryEntryAttrs::new()
                .with_type_(PspDirectoryEntryType::PmuPublicKey),
            firmware_blob_directory_name.join("Starship-PMU-FW.stkn"),
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
        .create_bhd_directory(AlignedLocation::try_from(0x24_0000).unwrap(), AlignedLocation::try_from(0x24_0000 + 0x8_0000).unwrap())
        .unwrap();
    // FIXME: Do our own Apcb.
    let apcb_source_file_name = match host_processor_generation {
        ProcessorGeneration::Milan => Path::new("amd-firmware").join("milan-ethx-1001").join("APCB_D4_DefaultRecovery.bin"),
        ProcessorGeneration::Rome => Path::new("amd-firmware").join("rome-ethx-100a").join("APCB_D4_DefaultRecovery.bin"),
        ProcessorGeneration::Naples => Path::new("amd-firmware").join("naples-diesel").join("APCB_D4_DefaultRecovery.bin"),
    };

    bhd_entry_add_from_file_with_custom_size(
        &mut bhd_directory,
        None,
        &match host_processor_generation {
            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup).with_sub_program(1),
            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
            ProcessorGeneration::Naples => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
        },
        Apcb::MAX_SIZE,
        apcb_source_file_name.as_path(),
        None,
    )
    .unwrap();

    bhd_directory
        .add_apob_entry(None, BhdDirectoryEntryType::Apob, 0x400_0000).unwrap();

    bhd_directory_add_reset_image(&mut bhd_directory, &opts.reset_image_filename).unwrap();
    bhd_directory_add_default_entries(&mut bhd_directory, &firmware_blob_directory_name).unwrap();

    bhd_entry_add_from_file_if_present(
        &mut bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Udimm_Imem.csbin"),
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
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Udimm_Dmem.csbin"),
        None,
    )
    .unwrap();

    bhd_entry_add_from_file_if_present(
        &mut bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(9)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Rdimm_Imem.csbin"),
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
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Rdimm_Dmem.csbin"),
        None,
    )
    .unwrap();

    bhd_entry_add_from_file_if_present(
        &mut bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(10)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Lrdimm_Imem.csbin"),
        None,
    )
    .unwrap();
    bhd_entry_add_from_file_if_present(
        &mut bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(10)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Lrdimm_Dmem.csbin"),
        None,
    )
    .unwrap();
    bhd_entry_add_from_file_if_present(
        &mut bhd_directory,
        None,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Udimm_Imem.csbin"),
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
        firmware_blob_directory_name.join("Appb_BIST_Ddr4_Udimm_Dmem.csbin"),
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
