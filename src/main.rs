
use core::cell::RefCell;
use std::env;
use std::fs::File;
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

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let filename = &args[1];
    let file = File::create(filename)?;
    file.set_len(16*1024*1024)?;
    let mut storage = FlashImage::new(file);
    let mut position: Location = 0;
    while position < 16*1024*1024 {
        FlashWrite::<0x1000, 0x2_0000>::erase_block(&mut storage, position).unwrap();
        position += 0x2_0000;
    }
    assert!(position == 16*1024*1024);
    eprintln!("BEFORE PROBLEMATIC AREA");
    let efs = match Efs::<_, 0x1000, 0x2_0000>::create(storage, ProcessorGeneration::Milan) {
        Ok(efs) => {
            efs
        },
        Err(e) => {
            eprintln!("Error on creation: {:?}", e);
            std::process::exit(1);
        }
    };
    eprintln!("STILL ALIVE");
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
