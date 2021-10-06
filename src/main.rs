use amd_efs::{
    BiosDirectory, BiosDirectoryEntryAttrs, BiosDirectoryEntryType, Efs, ProcessorGeneration,
    PspDirectory, PspDirectoryEntryAttrs, PspDirectoryEntryType,
    PspSoftFuseChain
};
use core::cell::RefCell;
use std::convert::TryInto;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
//use amd_efs::ProcessorGeneration;
use amd_flash::{Error, FlashRead, FlashWrite, Location, Result};

struct FlashImage {
    file: RefCell<File>,
}

impl<const READING_BLOCK_SIZE: usize, const ERASURE_BLOCK_SIZE: usize> FlashRead<READING_BLOCK_SIZE, ERASURE_BLOCK_SIZE> for FlashImage {
    fn read_block(&self, location: Location, buffer: &mut [u8; READING_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.read(buffer) {
            Ok(size) => {
                assert!(size == READING_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
    fn read_erasure_block(&self, location: Location, buffer: &mut [u8; ERASURE_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.read(buffer) {
            Ok(size) => {
                assert!(size == ERASURE_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
}

impl<const WRITING_BLOCK_SIZE: usize, const ERASURE_BLOCK_SIZE: usize>
    FlashWrite<WRITING_BLOCK_SIZE, ERASURE_BLOCK_SIZE> for FlashImage
{
    fn write_block(&self, location: Location, buffer: &[u8; WRITING_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.write(buffer) {
            Ok(size) => {
                assert!(size == WRITING_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
    fn erase_block(&self, location: Location) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        let buffer = [0xFFu8; ERASURE_BLOCK_SIZE];
        match file.write(&buffer[..]) {
            Ok(size) => {
                assert!(size == ERASURE_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
    fn erase_and_write_block(&self, location: Location, buffer: &[u8; ERASURE_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.write(&(*buffer)[..]) {
            Ok(size) => {
                assert!(size == ERASURE_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
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
const RW_BLOCK_SIZE: usize = 256;
const ERASURE_BLOCK_SIZE: usize = 0x1000;

// TODO: Allow size override.
fn psp_entry_add_from_file(
    directory: &mut PspDirectory<FlashImage, RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>,
    attrs: &PspDirectoryEntryAttrs,
    source_filename: &str,
) -> amd_efs::Result<()> {
    let file = File::open(source_filename).unwrap();
    let size: usize = file.metadata().unwrap().len().try_into().unwrap();
    let mut reader = BufReader::new(file);
    directory.add_blob_entry(None, attrs, size.try_into().unwrap(), &mut |buf: &mut [u8]| {
        reader
            .read(buf)
            .or(amd_efs::Result::Err(amd_efs::Error::Marshal))
    })?;
    Ok(())
}

fn bios_entry_add_from_file(
    directory: &mut BiosDirectory<FlashImage, RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>,
    attrs: &BiosDirectoryEntryAttrs,
    source_filename: &str,
    ram_destination_address: Option<u64>,
) -> amd_efs::Result<()> {
    let file = File::open(source_filename).unwrap();
    let size: usize = file.metadata().unwrap().len().try_into().unwrap();
    let mut reader = BufReader::new(file);
    directory.add_blob_entry(None, attrs, size.try_into().unwrap(), ram_destination_address, &mut |buf: &mut [u8]| {
        reader
            .read(buf)
            .or(amd_efs::Result::Err(amd_efs::Error::Marshal))
    })?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let filename = &args[1];
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filename)?;
    file.set_len(IMAGE_SIZE.into())?;
    let mut storage = FlashImage::new(file);
    let mut position: Location = 0;
    let block_size: Location = ERASURE_BLOCK_SIZE.try_into().unwrap();
    while position < IMAGE_SIZE {
        FlashWrite::<RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>::erase_block(&mut storage, position)
            .unwrap();
        position += block_size;
    }
    assert!(position == IMAGE_SIZE);
    let mut efs = match Efs::<_, RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>::create(
        storage,
        ProcessorGeneration::Milan,
    ) {
        Ok(efs) => efs,
        Err(e) => {
            eprintln!("Error on creation: {:?}", e);
            std::process::exit(1);
        }
    };
    let mut psp_directory = efs.create_psp_directory(0x12_0000, 0x24_0000).unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AmdPublicKey),
        "amd-firmware/milan/AmdPubKey_gn.tkn",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspBootloader),
        "amd-firmware/milan/PspBootLoader_gn.sbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspRecoveryBootloader),
        "amd-firmware/milan/PspRecoveryBootLoader_gn.sbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SmuOffChipFirmware8),
        "amd-firmware/milan/SmuFirmwareGn.csbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AblPublicKey),
        "amd-firmware/milan/PspABLFw_gn.stkn",
    )
    .unwrap();
    psp_directory.add_value_entry(
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspSoftFuseChain),
        PspSoftFuseChain::new().with_secure_debug_unlock(true).into(),
    )
    .unwrap();

    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SmuOffChipFirmware12),
        "amd-firmware/milan/SmuFirmware2Gn.csbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new()
            .with_type_(PspDirectoryEntryType::PspEarlySecureUnlockDebugImage),
        "amd-firmware/milan/SecureDebugUnlock_gn.sbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::WrappedIkek),
        "amd-firmware/milan/PspIkek_gn.bin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::PspTokenUnlockData),
        "amd-firmware/milan/SecureEmptyToken.bin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::SecurityPolicyBinary),
        "amd-firmware/milan/RsmuSecPolicy_gn.sbin",
    )
    .unwrap(); // FIXME: check blob
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::Mp5Firmware),
        "amd-firmware/milan/Mp5Gn.csbin",
    )
    .unwrap();
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::Abl0),
        "amd-firmware/milan/AgesaBootloader_U_prod_GN.csbin",
    )
    .unwrap();
    // TODO: SEV... but we don't use that.
    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::DxioPhySramFirmware),
        "amd-firmware/milan/GnPhyFw.sbin",
    )
    .unwrap();
    // TODO: psp_entry_add_from_file(&mut psp_directory, &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::DrtmTa), "amd-firmware/milan/PSP-DRTM_gn.sbin").unwrap()

    psp_entry_add_from_file(
        &mut psp_directory,
        &PspDirectoryEntryAttrs::new()
            .with_type_(PspDirectoryEntryType::PspBootloaderPublicKeysTable),
        "amd-firmware/milan/PSP-Key-DB_gn.sbin",
    )
    .unwrap();

    let mut bios_directory = efs
        .create_bios_directory(0x24_0000, 0x24_0000 + 0x8_0000)
        .unwrap();
    // FIXME: Do our own Apcb.
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::ApcbBackup)
            .with_sub_program(1),
        "amd-firmware/milan/APCB_GN_D4_DefaultRecovery.bin",
        None,
    )
    .unwrap();
    // TODO: twice (updatable apcb, eventlog apcb)
    bios_directory
        .add_apob_entry(None, BiosDirectoryEntryType::Apob, 0x3000_0000)
        .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::Bios)
            .with_reset_image(true)
            .with_copy_image(true),
        "reset.bin",
        Some(0x76c0_0000),
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(1)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(1)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(2)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(2)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(3)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(3)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(4)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(4)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(5)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(5)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(6)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(6)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B2.bin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B1.bin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B0.bin",
        None,
    )
    .unwrap();
    bios_entry_add_from_file(
        &mut bios_directory,
        &BiosDirectoryEntryAttrs::new()
            .with_type_(BiosDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_A0.bin",
        None,
    )
    .unwrap();

    //            println!("{:?}", efh);
    let psp_directory = match efs.psp_directory() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error on psp_directory: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("{:?}", psp_directory.header);
    for entry in psp_directory.entries() {
        println!("    {:?}", entry);
    }
    let bios_directories = match efs.bios_directories() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on bios_directory: {:?}", e);
            std::process::exit(1);
        }
    };
    for bios_directory in bios_directories {
        println!("{:?}", bios_directory.header);
        for entry in bios_directory.entries() {
            println!("    {:?}", entry);
        }
    }
    Ok(())
}
