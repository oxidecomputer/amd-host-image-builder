use amd_efs::{
    BhdDirectory, BhdDirectoryEntryAttrs, BhdDirectoryEntryType, Efs, ProcessorGeneration,
    PspDirectory, PspDirectoryEntryAttrs, PspDirectoryEntryType,
    PspSoftFuseChain
};
use core::cell::RefCell;
use core::convert::TryFrom;
use core::convert::TryInto;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
//use amd_efs::ProcessorGeneration;
use amd_flash::{Error, FlashRead, FlashWrite, Location, ErasableLocation, Result};

struct FlashImage {
    file: RefCell<File>,
}

impl<const ERASABLE_BLOCK_SIZE: usize> FlashRead<ERASABLE_BLOCK_SIZE> for FlashImage {
    fn read_exact(&self, location: Location, buffer: &mut [u8]) -> Result<usize> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.read_exact(buffer) {
            Ok(()) => {
                Ok(buffer.len())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
    fn read_erasable_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>, buffer: &mut [u8; ERASABLE_BLOCK_SIZE]) -> Result<()> {
        let location = Location::from(location);
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.read(buffer) {
            Ok(size) => {
                assert!(size == ERASABLE_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
}

impl<const ERASABLE_BLOCK_SIZE: usize>
    FlashWrite<ERASABLE_BLOCK_SIZE> for FlashImage
{
    fn erase_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>) -> Result<()> {
        let location = Location::from(location);
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        let buffer = [0xFFu8; ERASABLE_BLOCK_SIZE];
        match file.write(&buffer[..]) {
            Ok(size) => {
                assert!(size == ERASABLE_BLOCK_SIZE);
                Ok(())
            }
            Err(e) => {
                return Err(Error::Io);
            }
        }
    }
    fn erase_and_write_block(&self, location: ErasableLocation<ERASABLE_BLOCK_SIZE>, buffer: &[u8; ERASABLE_BLOCK_SIZE]) -> Result<()> {
        let location = Location::from(location);
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {}
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.write(&(*buffer)[..]) {
            Ok(size) => {
                assert!(size == ERASABLE_BLOCK_SIZE);
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
const ERASABLE_BLOCK_SIZE: usize = 0x1000;
type AlignedLocation = ErasableLocation<ERASABLE_BLOCK_SIZE>;

// TODO: Allow size override.
fn psp_entry_add_from_file(
    directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
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

fn bhd_entry_add_from_file(
    directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
    attrs: &BhdDirectoryEntryAttrs,
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
    let mut position: AlignedLocation = 0.try_into().unwrap();
    let block_size: Location = ERASABLE_BLOCK_SIZE.try_into().unwrap();
    while Location::from(position) < IMAGE_SIZE {
        FlashWrite::<ERASABLE_BLOCK_SIZE>::erase_block(&mut storage, position)
            .unwrap();
        position = position.advance(ERASABLE_BLOCK_SIZE).unwrap();
    }
    assert!(Location::from(position) == IMAGE_SIZE);
    let mut efs = match Efs::<_, ERASABLE_BLOCK_SIZE>::create(
        storage,
        ProcessorGeneration::Milan,
    ) {
        Ok(efs) => efs,
        Err(e) => {
            eprintln!("Error on creation: {:?}", e);
            std::process::exit(1);
        }
    };
    let mut psp_directory = efs.create_psp_directory(AlignedLocation::try_from(0x12_0000).unwrap(), AlignedLocation::try_from(0x24_0000).unwrap()).unwrap();
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

    let mut bhd_directory = efs
        .create_bhd_directory(AlignedLocation::try_from(0x24_0000).unwrap(), AlignedLocation::try_from(0x24_0000 + 0x8_0000).unwrap())
        .unwrap();
    // FIXME: Do our own Apcb.
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::ApcbBackup)
            .with_sub_program(1),
        "amd-firmware/milan/APCB_GN_D4_DefaultRecovery.bin",
        None,
    )
    .unwrap();
    // TODO: twice (updatable apcb, eventlog apcb)
    bhd_directory
        .add_apob_entry(None, BhdDirectoryEntryType::Apob, 0x3000_0000)
        .unwrap();

    /*bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::Bios)
            .with_reset_image(true)
            .with_copy_image(true),
        "reset.bin",
        Some(0x76c0_0000),
    )
    .unwrap();*/

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(1)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(1)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(2)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(2)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(3)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(3)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_1D_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(4)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(4)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(5)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(5)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(6)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(6)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_2D_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareInstructions)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::PmuFirmwareData)
            .with_instance(8)
            .with_sub_program(1),
        "amd-firmware/milan/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin",
        None,
    )
    .unwrap();

    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B2.bin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B1.bin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
            .with_instance(0),
        "amd-firmware/milan/UcodePatch_GN_B0.bin",
        None,
    )
    .unwrap();
    bhd_entry_add_from_file(
        &mut bhd_directory,
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::MicrocodePatch)
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
    let bhd_directories = match efs.bhd_directories() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on bhd_directory: {:?}", e);
            std::process::exit(1);
        }
    };
    for bhd_directory in bhd_directories {
        println!("{:?}", bhd_directory.header);
        for entry in bhd_directory.entries() {
            println!("    {:?}", entry);
        }
    }
    Ok(())
}
