
use core::cell::RefCell;
use std::convert::TryInto;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use amd_efs::{Efs, ProcessorGeneration};
//use amd_efs::ProcessorGeneration;
use amd_flash::{FlashRead, FlashWrite, Location, Result, Error};

struct FlashImage {
    file: RefCell<File>,
}

impl<const READING_BLOCK_SIZE: usize> FlashRead<READING_BLOCK_SIZE> for FlashImage {
    fn read_block(&self, location: Location, buffer: &mut [u8; READING_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {
            },
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.read(buffer) {
            Ok(size) => {
                assert!(size == READING_BLOCK_SIZE);
                Ok(())
            },
            Err(e) => {
                return Err(Error::Io);
            },
        }
    }
}

impl<const WRITING_BLOCK_SIZE: usize, const ERASURE_BLOCK_SIZE: usize> FlashWrite<WRITING_BLOCK_SIZE, ERASURE_BLOCK_SIZE> for FlashImage {
    fn write_block(&mut self, location: Location, buffer: &[u8; WRITING_BLOCK_SIZE]) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {
            },
            Err(e) => {
                return Err(Error::Io);
            }
        }
        match file.write(buffer) {
            Ok(size) => {
                assert!(size == WRITING_BLOCK_SIZE);
                Ok(())
            },
            Err(e) => {
                return Err(Error::Io);
            },
        }
    }
    fn erase_block(&mut self, location: Location) -> Result<()> {
        let mut file = self.file.borrow_mut();
        match file.seek(SeekFrom::Start(location.into())) {
            Ok(_) => {
            },
            Err(e) => {
                return Err(Error::Io);
            }
        }
        let buffer = [0xFFu8; ERASURE_BLOCK_SIZE];
        match file.write(&buffer) {
            Ok(size) => {
                assert!(size == ERASURE_BLOCK_SIZE);
                Ok(())
            },
            Err(e) => {
                return Err(Error::Io);
            },
        }
    }
}

impl FlashImage {
    fn new(file: File) -> Self {
        Self {
            file: RefCell::new(file)
        }
    }
}

const IMAGE_SIZE: u32 = 16*1024*1024;
const RW_BLOCK_SIZE: usize = 0x1000;
const ERASURE_BLOCK_SIZE: usize = 0x2_0000;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let filename = &args[1];
    let file = OpenOptions::new().read(true).write(true).create(true).open(filename)?;
    file.set_len(IMAGE_SIZE.into())?;
    let mut storage = FlashImage::new(file);
    let mut position: Location = 0;
    let block_size: Location = ERASURE_BLOCK_SIZE.try_into().unwrap();
    while position < IMAGE_SIZE {
        FlashWrite::<RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>::erase_block(&mut storage, position).unwrap();
        position += block_size;
    }
    assert!(position == IMAGE_SIZE);
    let mut efs = match Efs::<_, RW_BLOCK_SIZE, ERASURE_BLOCK_SIZE>::create(storage, ProcessorGeneration::Milan) {
        Ok(efs) => {
            efs
        },
        Err(e) => {
            eprintln!("Error on creation: {:?}", e);
            std::process::exit(1);
        }
    };
    efs.create_psp_directory(0x12_0000, 0x24_0000).unwrap();
    efs.create_bios_directory(0x24_0000, 0x24_0000 + 0x4_0000).unwrap();
//            println!("{:?}", efh);
    let psp_directory = match efs.psp_directory() {
        Ok(v) => {
            v
        },
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
        Ok(d) => {
            d
        },
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
