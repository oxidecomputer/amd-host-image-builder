use amd_flash::{
    ErasableLocation, FlashAlign, FlashRead, FlashWrite, Location,
};
use std::cell::RefCell;
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
    ) -> amd_flash::Result<()> {
        let filename = &self.filename;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_flash::Error::Io
        })?;
        file.read_exact(buffer).map_err(|e| {
            eprintln!("Error reading from flash image {filename:?}: {e:?}");
            amd_flash::Error::Io
        })
    }
}

impl FlashAlign for FlashImage {
    fn erasable_block_size(&self) -> usize {
        self.erasable_block_size
    }
}

impl FlashWrite for FlashImage {
    fn erase_block(&self, location: ErasableLocation) -> amd_flash::Result<()> {
        let filename = &self.filename;
        let erasable_block_size = self.erasable_block_size();
        let location = self.location(location)?;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_flash::Error::Io
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
                Err(amd_flash::Error::Io)
            }
        }
    }
    fn erase_and_write_block(
        &self,
        location: ErasableLocation,
        buffer: &[u8],
    ) -> amd_flash::Result<()> {
        let filename = &self.filename;
        let erasable_block_size = self.erasable_block_size;
        if buffer.len() > erasable_block_size {
            return Err(amd_flash::Error::Programmer);
        }
        let location = self.location(location)?;
        let mut file = self.file.borrow_mut();
        file.seek(SeekFrom::Start(location.into())).map_err(|e| {
            eprintln!("Error seeking in flash image {filename:?}: {e:?}");
            amd_flash::Error::Io
        })?;
        let mut xbuffer = self.buffer.borrow_mut();
        xbuffer.fill(0xff);
        xbuffer[..buffer.len()].copy_from_slice(buffer);
        let size = file.write(&xbuffer).map_err(|e| {
            eprintln!("Error writing to flash image {filename:?}: {e:?}");
            amd_flash::Error::Io
        })?;
        assert!(size == erasable_block_size);
        Ok(())
    }
}

impl FlashImage {
    pub fn create(
        filename: &Path,
        erasable_block_size: usize,
    ) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(filename)?;
        file.set_len(crate::static_config::IMAGE_SIZE.into())?;
        assert!(erasable_block_size.is_power_of_two());
        let result = Self {
            file: RefCell::new(file),
            filename: filename.to_path_buf(),
            erasable_block_size,
            buffer: RefCell::new(
                std::iter::repeat(0xff).take(erasable_block_size).collect(),
            ),
        };
        result.erase()?;
        Ok(result)
    }
    fn erase(&self) -> std::io::Result<()> {
        let filename = &self.filename;
        let flash_to_io_error = |e: amd_flash::Error| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Flash error: {e:?} in file {filename:?}"),
            )
        };
        let erasable_block_size = self.erasable_block_size;
        let mut position = self
            .erasable_location(0)
            .ok_or(amd_flash::Error::Alignment)
            .map_err(flash_to_io_error)?;
        while Location::from(position) < crate::static_config::IMAGE_SIZE {
            FlashWrite::erase_block(self, position)
                .map_err(flash_to_io_error)?;
            position = position
                .advance(erasable_block_size)
                .map_err(flash_to_io_error)?;
        }
        assert!(Location::from(position) == crate::static_config::IMAGE_SIZE);
        Ok(())
    }
}
