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
use std::path::Path;
use std::path::PathBuf;
use amd_apcb::Apcb;
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

fn psp_entry_add_from_file(
    directory: &mut PspDirectory<FlashImage, ERASABLE_BLOCK_SIZE>,
    payload_position: Option<ErasableLocation<ERASABLE_BLOCK_SIZE>>,
    attrs: &PspDirectoryEntryAttrs,
    source_filename: PathBuf,
) -> amd_efs::Result<()> {
    let file = File::open(source_filename).unwrap();
    let size: usize = file.metadata().unwrap().len().try_into().unwrap();
    let mut reader = BufReader::new(file);
    directory.add_blob_entry(payload_position, attrs, size.try_into().unwrap(), &mut |buf: &mut [u8]| {
        reader
            .read(buf)
            .or(amd_efs::Result::Err(amd_efs::Error::Marshal))
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
    directory.add_blob_entry(payload_position, attrs, size.try_into().unwrap(), ram_destination_address, &mut |buf: &mut [u8]| {
        reader
            .read(buf)
            .or(amd_efs::Result::Err(amd_efs::Error::Marshal))
    })?;
    Ok(())
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

fn bhd_directory_add_default_entries(bhd_directory: &mut BhdDirectory<FlashImage, ERASABLE_BLOCK_SIZE>, firmware_blob_directory_name: &PathBuf) -> amd_efs::Result<()> {
    bhd_directory
        .add_apob_entry(None, BhdDirectoryEntryType::Apob, 0x3000_0000)?;

    bhd_entry_add_from_file(
        bhd_directory,
        Some(0xd00000.try_into().unwrap()), // TODO: Could also be None--works.
        &BhdDirectoryEntryAttrs::new()
            .with_type_(BhdDirectoryEntryType::Bios)
            .with_reset_image(true)
            .with_copy_image(true),
        Path::new("nanobl-rs-0x7ffc_d000.bin").to_path_buf(),
        Some(0x7ffc_d000),
    )?;

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
    let host_processor_generation = ProcessorGeneration::Milan;
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
    psp_entry_add_from_file(
        &mut psp_directory,
        None,
        &PspDirectoryEntryAttrs::new().with_type_(PspDirectoryEntryType::AmdSecureDebugKey),
        firmware_blob_directory_name.join("SecureDebugToken.stkn"),
    ).unwrap(); // XXX cannot remove
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
///* removed    psp_entry_add_from_file(
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

    let firmware_blob_directory_name = match host_processor_generation {
        ProcessorGeneration::Milan => Path::new("amd-firmware").join("milan"),
        ProcessorGeneration::Rome => Path::new("amd-firmware").join("rome"),
    };
    let mut bhd_directory = efs
        .create_bhd_directory(AlignedLocation::try_from(0x24_0000).unwrap(), AlignedLocation::try_from(0x24_0000 + 0x8_0000).unwrap())
        .unwrap();
    // FIXME: Do our own Apcb.
    let apcb_source_file_name = match host_processor_generation {
        ProcessorGeneration::Milan => Path::new("amd-firmware").join("milan-ethx-1001").join("APCB_D4_DefaultRecovery.bin"),
        ProcessorGeneration::Rome => Path::new("amd-firmware").join("rome-ethx-100a").join("APCB_D4_DefaultRecovery.bin"),
    };

    bhd_entry_add_from_file_with_custom_size(
        &mut bhd_directory,
        None,
        &match host_processor_generation {
            ProcessorGeneration::Milan => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup).with_sub_program(1),
            ProcessorGeneration::Rome => BhdDirectoryEntryAttrs::new().with_type_(BhdDirectoryEntryType::ApcbBackup),
        },
        Apcb::MAX_SIZE,
        apcb_source_file_name.as_path(),
        None,
    )
    .unwrap();

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
