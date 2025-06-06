use amd_efs::flash::{
    ErasableLocation, FlashAlign, FlashRead, FlashWrite, Location,
};
use std::cell::RefCell;
use std::convert::TryFrom;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

pub struct FlashImage {
    file: RefCell<File>,
    filename: PathBuf,
    erasable_block_size: usize,
    buffer: RefCell<Vec<u8>>,
}

impl FlashRead for FlashImage {
    fn read_exact(
        &self,
        location: Location,
        buffer: &mut [u8],
    ) -> amd_efs::flash::Result<()> {
        let filename = &self.filename;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_efs::flash::Error::Io(amd_efs::flash::IoError::Read {
                start: location,
                size: buffer.len(),
            })
        })?;
        file.read_exact(buffer).map_err(|e| {
            eprintln!("Error reading from flash image {filename:?}: {e:?}");
            amd_efs::flash::Error::Io(amd_efs::flash::IoError::Read {
                start: location,
                size: buffer.len(),
            })
        })
    }
}

impl FlashAlign for FlashImage {
    fn erasable_block_size(&self) -> usize {
        self.erasable_block_size
    }
}

impl FlashWrite for FlashImage {
    fn erase_block(
        &self,
        location: ErasableLocation,
    ) -> amd_efs::flash::Result<()> {
        let filename = &self.filename;
        let erasable_block_size = self.erasable_block_size();
        let location = self.location(location)?;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_efs::flash::Error::Io(amd_efs::flash::IoError::Erase {
                start: location,
                size: erasable_block_size,
            })
        })?;
        let mut buffer = self.buffer.borrow_mut();
        buffer.fill(0xff);
        match file.write(&buffer[..]) {
            Ok(size) => {
                assert!(size == erasable_block_size);
                Ok(())
            }
            Err(e) => {
                eprintln!("Error writing to flash image {filename:?}: {e:?}");
                Err(amd_efs::flash::Error::Io(amd_efs::flash::IoError::Write {
                    start: location,
                    size: buffer.len(),
                }))
            }
        }
    }
    fn erase_and_write_block(
        &self,
        location: ErasableLocation,
        buffer: &[u8],
    ) -> amd_efs::flash::Result<()> {
        let filename = &self.filename;
        let erasable_block_size = self.erasable_block_size;
        if buffer.len() > erasable_block_size {
            panic!("passed buffer length is bigger than erase block size");
        }
        let location = self.location(location)?;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_efs::flash::Error::Io(amd_efs::flash::IoError::Write {
                start: location,
                size: buffer.len(),
            })
        })?;
        let mut xbuffer = self.buffer.borrow_mut();
        xbuffer.fill(0xff);
        xbuffer[..buffer.len()].copy_from_slice(buffer);
        let size = file.write(&xbuffer).map_err(|e| {
            eprintln!("Error writing to flash image {filename:?}: {e:?}");
            amd_efs::flash::Error::Io(amd_efs::flash::IoError::Write {
                start: location,
                size: buffer.len(),
            })
        })?;
        assert!(size == erasable_block_size);
        Ok(())
    }
}

impl FlashImage {
    pub fn create(
        filename: &Path,
        image_size: u32,
        erasable_block_size: usize,
    ) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(filename)?;
        file.set_len(image_size.into())?;
        assert!(erasable_block_size.is_power_of_two());
        let result = Self {
            file: RefCell::new(file),
            filename: filename.to_path_buf(),
            erasable_block_size,
            buffer: RefCell::new(
                std::iter::repeat_n(0xff, erasable_block_size).collect(),
            ),
        };
        Ok(result)
    }
    pub(crate) fn erase(&self) -> std::io::Result<()> {
        let filename = &self.filename;
        let file_size = u32::try_from(self.file_size()?).unwrap();
        let flash_to_io_error = |e: amd_efs::flash::Error| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Flash error: {e:?} in file {filename:?}"),
            )
        };
        let erasable_block_size = self.erasable_block_size;
        let mut position =
            self.erasable_location(0).map_err(flash_to_io_error)?;
        while Location::from(position) < file_size {
            FlashWrite::erase_block(self, position)
                .map_err(flash_to_io_error)?;
            position = position
                .advance(erasable_block_size)
                .map_err(flash_to_io_error)?;
        }
        assert!(Location::from(position) == file_size);
        Ok(())
    }
    pub fn load(filename: &Path) -> std::io::Result<Self> {
        const B: usize = 1;
        let erasable_block_size = 8192 * B;
        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .create(false)
            .open(filename)?;
        let result = Self {
            file: RefCell::new(file),
            filename: filename.to_path_buf(),
            erasable_block_size,
            buffer: RefCell::new(
                std::iter::repeat_n(0xff, erasable_block_size).collect(),
            ),
        };
        Ok(result)
    }
    pub fn file_size(&self) -> std::io::Result<u64> {
        Ok(self.file.borrow().metadata()?.len())
    }
}
