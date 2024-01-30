use amd_apcb::{Apcb, ApcbIoOptions};

use amd_efs::{
    AddressMode, BhdDirectory, BhdDirectoryEntry, BhdDirectoryEntryType,
    BhdDirectoryHeader, DirectoryAdditionalInfo, DirectoryEntry, Efs,
    ProcessorGeneration, PspDirectory, PspDirectoryEntry,
    PspDirectoryEntryType, PspDirectoryHeader, ValueOrLocation,
};
use amd_host_image_builder_config::{
    Error, Result, SerdeBhdDirectory, SerdeBhdDirectoryEntry,
    SerdeBhdDirectoryEntryAttrs, SerdeBhdDirectoryEntryBlob,
    SerdeBhdDirectoryVariant, SerdeBhdEntry, SerdeBhdSource, SerdePspDirectory,
    SerdePspDirectoryEntry, SerdePspDirectoryEntryAttrs,
    SerdePspDirectoryEntryBlob, SerdePspDirectoryVariant, SerdePspEntry,
    SerdePspEntrySource, TryFromSerdeDirectoryEntryWithContext,
};
use bytesize::ByteSize;
use core::convert::TryFrom;
use core::convert::TryInto;
use static_assertions::const_assert;
use std::cmp::min;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

mod static_config;
use amd_flash::allocators::{ArenaFlashAllocator, FlashAllocate};

use amd_flash::{
    ErasableLocation, ErasableRange, FlashAlign, FlashRead, FlashWrite,
    Location,
};
use amd_host_image_builder_config::SerdeConfig;

#[test]
fn test_bitfield_serde() {
    let config = r#"{
        "spi_block_size": 5,
        "max_size": 2,
        "base_address": 3,
        "address_mode": "PhysicalAddress"
}"#;
    use amd_efs::DirectoryAdditionalInfo;
    let result: DirectoryAdditionalInfo = json5::from_str(config).unwrap();
    assert_eq!(result.address_mode(), AddressMode::PhysicalAddress);
}

mod hole;
use hole::Hole;

mod images;
use images::FlashImage;

/// Open SOURCE_FILENAME and checks its size.
/// If TARGET_SIZE is given, make sure the file is at most as big as that.
/// If file is too big, error out.
/// Otherwise, return the size to use for the entry payload.
fn size_file(
    source_filename: &Path,
    target_size: Option<u32>,
) -> amd_efs::Result<(File, u32)> {
    let file = match File::open(source_filename) {
        Ok(f) => f,
        Err(e) => {
            panic!("Could not open file {:?}: {}", source_filename, e);
        }
    };
    let filesize: usize = file
        .metadata()
        .map_err(|_| amd_efs::Error::Io(amd_flash::Error::Io))?
        .len()
        .try_into()
        .map_err(|_| amd_efs::Error::Io(amd_flash::Error::Io))?;
    match target_size {
        Some(x) => {
            if filesize > x as usize {
                panic!("Configuration specifies slot size {} but contents {:?} have size {}. The contents do not fit.", x, source_filename, filesize);
            } else {
                Ok((file, x))
            }
        }
        None => Ok((
            file,
            filesize
                .try_into()
                .map_err(|_| amd_efs::Error::DirectoryPayloadRangeCheck)?,
        )),
    }
}

/// Reads the file named SOURCE_FILENAME, finds the version field in there (if any) and returns
/// its value.
/// In case of error (file can't be read, version field not found, ...),
/// returns None.
fn abl_file_version(source_filename: &Path) -> Option<u32> {
    // Note: This does work on Rome starting with Rome 1.0.0.a.
    let (file, _size) = size_file(source_filename, None).ok()?;
    let mut source = BufReader::new(file);
    let mut header: [u8; 0x110] = [0; 0x110];
    source.read_exact(&mut header).ok()?;
    let ver_raw = <[u8; 4]>::try_from(&header[0x60..0x64]).ok()?;
    let ver = u32::from_le_bytes(ver_raw);
    if ver != 0 {
        return Some(ver);
    }

    let ver_header_loc_raw = <[u8; 4]>::try_from(&header[0x104..0x108]).ok()?;
    let ver_header_loc = u32::from_le_bytes(ver_header_loc_raw).into();
    source.seek(SeekFrom::Start(ver_header_loc)).ok()?;
    let mut header: [u8; 0x64] = [0; 0x64]; // or more, I guess
    source.read_exact(&mut header).ok()?;
    let ver_raw = <[u8; 4]>::try_from(&header[0x60..0x64]).ok()?;
    let ver = u32::from_le_bytes(ver_raw);
    (ver != 0).then_some(ver)
}

/// Reads the file named SOURCE_FILENAME, finds the version field in there (if any) and returns
/// its value.
/// In case of error (file can't be read, version field not found, ...),
/// returns None.
fn smu_file_version(source_filename: &Path) -> Option<(u8, u8, u8, u8)> {
    let (file, _size) = size_file(source_filename, None).ok()?;
    let mut source = BufReader::new(file);
    let mut header: [u8; 0x100] = [0; 0x100];
    source.read_exact(&mut header).ok()?;
    let ver_raw = <[u8; 4]>::try_from(&header[0x60..0x64]).ok()?;
    (ver_raw[2] != 0)
        .then_some((ver_raw[3], ver_raw[2], ver_raw[1], ver_raw[0]))
}

fn elf_symbol(
    binary: &goblin::elf::Elf,
    key: &str,
) -> Option<goblin::elf::Sym> {
    for sym in &binary.syms {
        let ix = sym.st_name;
        if ix != 0 && &binary.strtab[sym.st_name] == key {
            return Some(sym);
        }
    }
    None
}

fn bhd_directory_add_reset_image(
    reset_image_filename: &Path,
) -> Result<(BhdDirectoryEntry, Vec<u8>)> {
    let buffer = fs::read(reset_image_filename).map_err(Error::Io)?;
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
            if binary.header.e_type != goblin::elf::header::ET_EXEC
                || binary.header.e_machine != goblin::elf::header::EM_X86_64
                || binary.header.e_version
                    < goblin::elf::header::EV_CURRENT.into()
            {
                return Err(Error::IncompatibleExecutable);
            }
            for header in &binary.program_headers {
                if header.p_type == goblin::elf::program_header::PT_LOAD {
                    //eprintln!("PROG {:x?}", header);
                    if header.p_memsz == 0 {
                        continue;
                    }
                    if destination_origin.is_none() {
                        // Note: File is sorted by p_vaddr.
                        destination_origin = Some(header.p_vaddr);
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
                            holesz += (header.p_vaddr - last_vaddr) as usize;
                        }
                        if holesz > 0 {
                            //eprintln!("hole: {:x}", holesz);
                            iov = Box::new(iov.chain(Hole::new(holesz)))
                                as Box<dyn Read>;
                            totalsz += holesz;
                            holesz = 0;
                        }
                        let chunk = &buffer[header.p_offset as usize
                            ..(header.p_offset + header.p_filesz) as usize];
                        //eprintln!("chunk: {:x} @ {:x}", header.p_filesz, header.p_offset);
                        iov = Box::new(iov.chain(chunk)) as Box<dyn Read>;
                        totalsz += header.p_filesz as usize;
                        if header.p_memsz > header.p_filesz {
                            holesz +=
                                (header.p_memsz - header.p_filesz) as usize;
                        }
                        last_vaddr = header.p_vaddr + header.p_memsz;
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
            if sloader
                != destination_origin.ok_or(Error::IncompatibleExecutable)?
            {
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
            if binary.header.e_entry
                != last_vaddr
                    .checked_sub(0x10)
                    .ok_or(Error::IncompatibleExecutable)?
            {
                return Err(Error::IncompatibleExecutable);
            }
            if last_vaddr & 0xffff != 0 {
                return Err(Error::IncompatibleExecutable);
            }
        }
        _ => {
            destination_origin = Some(
                0x8000_0000u64
                    .checked_sub(buffer.len() as u64)
                    .ok_or(Error::ImageTooBig)?,
            );
            iov = Box::new(buffer.as_slice()) as Box<dyn Read>;
            sz = buffer.len();
        }
    }

    if destination_origin.is_none() {
        eprintln!("Warning: No destination in RAM specified for Reset image.");
    }

    let entry = BhdDirectoryEntry::new_payload(
        AddressMode::EfsRelativeOffset,
        BhdDirectoryEntryType::Bios,
        Some(
            sz.try_into()
                .map_err(|_| amd_efs::Error::DirectoryPayloadRangeCheck)?,
        ),
        None,
        destination_origin,
    )?
    .with_reset_image(true)
    .with_copy_image(true)
    .build();
    // Write write_all
    let mut result = Vec::<u8>::new();
    std::io::copy(&mut iov, &mut result).unwrap();
    Ok((entry, result))
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "amd-host-image-builder",
    about = "Build host flash image for AMD Zen CPUs."
)]
enum Opts {
    Generate {
        #[structopt(short = "o", long = "output-file", parse(from_os_str))]
        output_filename: PathBuf,

        #[structopt(
            default_value = "32 MiB",
            short = "s",
            long = "output-size",
            parse(try_from_str = ByteSize::from_str)
        )]
        output_size: ByteSize,

        #[structopt(short = "r", long = "reset-image", parse(from_os_str))]
        reset_image_filename: Option<PathBuf>,

        #[structopt(short = "c", long = "config", parse(from_os_str))]
        efs_configuration_filename: PathBuf,

        #[structopt(short = "B", long = "blobdir", parse(from_os_str))]
        blobdirs: Vec<PathBuf>,

        #[structopt(short = "v", long = "verbose")]
        verbose: bool,
    },
    Dump {
        #[structopt(short = "i", long = "existing-file", parse(from_os_str))]
        input_filename: PathBuf,

        #[structopt(
            short = "b",
            long = "blob-dump-directory",
            parse(from_os_str)
        )]
        blob_dump_dirname: Option<PathBuf>,
    },
}

//#[allow(clippy::type_complexity)]
fn create_psp_directory<T: FlashRead + FlashWrite>(
    psp_type: [u8; 4],
    psp_directory_location: Option<Location>,
    psp_raw_entries: &mut Vec<(
        PspDirectoryEntry,
        Option<Location>,
        Option<Vec<u8>>,
    )>,
    psp_directory_address_mode: AddressMode,
    storage: &FlashImage,
    allocator: &mut impl FlashAllocate,
    efs: &mut Efs<T>,
    output_filename: &Path,
) -> std::io::Result<(PspDirectory, ErasableRange, Option<ErasableLocation>)> {
    let filename = output_filename;
    let efs_to_io_error = |e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("EFS error: {e:?} in file {filename:?}"),
        )
    };

    let mut first_payload_range_beginning: Option<ErasableLocation> = None;

    // Here we know how big the directory is gonna be.

    let mut psp_directory_size =
        PspDirectory::minimal_directory_size(psp_raw_entries.len())
            .map_err(efs_to_io_error)?;

    if psp_directory_size % DirectoryAdditionalInfo::UNIT != 0 {
        psp_directory_size += DirectoryAdditionalInfo::UNIT
            - (psp_directory_size % DirectoryAdditionalInfo::UNIT); // FIXME overflow
        assert!(psp_directory_size % DirectoryAdditionalInfo::UNIT == 0);
    }

    // Traverse psp_raw_entries and update SOURCE accordingly

    for (entry, source_override, blob_body) in psp_raw_entries.iter_mut() {
        eprintln!("PSP {:?}", entry);
        if let Some(blob_body) = blob_body {
            let source = if let Some(source_override) = source_override {
                if first_payload_range_beginning.is_none() {
                    first_payload_range_beginning = Some(
                        storage.erasable_location(*source_override)
                        .expect("Expected the location to be a multiple of erase block size"),
                    )
                }
                *source_override
            } else {
                let destination =
                    allocator.take_at_least(blob_body.len()).unwrap();
                if first_payload_range_beginning.is_none() {
                    first_payload_range_beginning = Some(destination.beginning)
                }
                Location::from(destination.beginning)
            };
            // TODO set_size maybe
            entry
                .set_source(
                    AddressMode::DirectoryRelativeOffset,
                    ValueOrLocation::EfsRelativeOffset(source),
                )
                .map_err(efs_to_io_error)?;
        }
    }
    eprintln!("PSP preparation done");

    let psp_entries = psp_raw_entries
        .iter()
        .map(|(raw_entry, _, _)| *raw_entry)
        .collect::<Vec<PspDirectoryEntry>>();
    let psp_directory_range = match psp_directory_location {
        Some(x) => ErasableRange {
            beginning: storage.erasable_location(x).unwrap(),
            end: storage
                .erasable_location(
                    (Location::from(x) as usize + psp_directory_size)
                        .try_into()
                        .unwrap(),
                )
                .unwrap(),
        },
        None => allocator.take_at_least(psp_directory_size).unwrap(),
    };
    // FIXME beginning = 401408
    let psp_directory_beginning = psp_directory_range.beginning;
    let psp_directory_end = psp_directory_range.end;
    let psp_directory = efs
        .create_psp_directory1(
            psp_type,
            psp_directory_beginning,
            psp_directory_end,
            psp_directory_address_mode,
            &psp_entries,
        )
        .map_err(efs_to_io_error)?;

    if let Some(x) = first_payload_range_beginning {
        let mut x = Location::from(x);
        if x % (DirectoryAdditionalInfo::UNIT as Location) != 0 {
            x -= x % (DirectoryAdditionalInfo::UNIT as Location);
            assert!(
                Location::from(x) % (DirectoryAdditionalInfo::UNIT as Location)
                    == 0
            );
        }
        first_payload_range_beginning =
            Some(storage.erasable_location(x).unwrap());
    }

    eprintln!("PSP payloads done");
    Ok((psp_directory, psp_directory_range, first_payload_range_beginning))
}

//#[allow(clippy::type_complexity)]
fn create_bhd_directory<T: FlashRead + FlashWrite>(
    bhd_type: [u8; 4],
    bhd_directory_location: Option<Location>,
    bhd_raw_entries: &mut Vec<(
        BhdDirectoryEntry,
        Option<Location>,
        Option<Vec<u8>>,
    )>,
    bhd_directory_address_mode: AddressMode,
    storage: &FlashImage,
    allocator: &mut impl FlashAllocate,
    efs: &mut Efs<T>,
    output_filename: &Path,
) -> std::io::Result<(BhdDirectory, ErasableRange, Option<ErasableLocation>)> {
    let filename = output_filename;
    let efs_to_io_error = |e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("EFS error: {e:?} in file {filename:?}"),
        )
    };

    let mut first_payload_range_beginning: Option<ErasableLocation> = None;

    // Here we know how big the directory is gonna be.

    let mut bhd_directory_size =
        BhdDirectory::minimal_directory_size(bhd_raw_entries.len())
            .map_err(efs_to_io_error)?;

    if bhd_directory_size % DirectoryAdditionalInfo::UNIT != 0 {
        bhd_directory_size += DirectoryAdditionalInfo::UNIT
            - (bhd_directory_size % DirectoryAdditionalInfo::UNIT); // FIXME overflow
        assert!(bhd_directory_size % DirectoryAdditionalInfo::UNIT == 0);
    }

    // Traverse bhd_raw_entries and update SOURCE accordingly

    for (entry, source_override, blob_body) in bhd_raw_entries.iter_mut() {
        if let Some(blob_body) = blob_body {
            let source = if let Some(source_override) = source_override {
                if first_payload_range_beginning.is_none() {
                    first_payload_range_beginning = Some(
                        storage.erasable_location(*source_override).unwrap(),
                    )
                }
                *source_override
            } else {
                let destination =
                    allocator.take_at_least(blob_body.len()).unwrap();
                if first_payload_range_beginning.is_none() {
                    first_payload_range_beginning = Some(destination.beginning)
                }
                Location::from(destination.beginning)
            };
            // Required because of BhdDirectoryEntry::new_payload() for reset image.
            entry.set_size(Some(
                blob_body
                    .len()
                    .try_into()
                    .map_err(|_| amd_efs::Error::DirectoryPayloadRangeCheck)
                    .map_err(efs_to_io_error)?,
            ));
            entry
                .set_source(
                    AddressMode::DirectoryRelativeOffset,
                    ValueOrLocation::EfsRelativeOffset(source),
                )
                .map_err(efs_to_io_error)?;
        }
    }

    let bhd_entries = bhd_raw_entries
        .iter()
        .map(|(raw_entry, _, _)| *raw_entry)
        .collect::<Vec<BhdDirectoryEntry>>();
    let bhd_directory_range = match bhd_directory_location {
        Some(x) => ErasableRange {
            beginning: storage.erasable_location(x).unwrap(),
            end: storage
                .erasable_location(
                    (Location::from(x) as usize + bhd_directory_size)
                        .try_into()
                        .unwrap(),
                )
                .unwrap(),
        },
        None => allocator.take_at_least(bhd_directory_size).unwrap(),
    };
    // beginning 3088384
    let bhd_directory_beginning = bhd_directory_range.beginning;
    let bhd_directory_end = bhd_directory_range.end;
    let bhd_directory = efs
        .create_bhd_directory1(
            bhd_type,
            bhd_directory_beginning,
            bhd_directory_end,
            bhd_directory_address_mode,
            &bhd_entries,
        )
        .map_err(efs_to_io_error)?;

    Ok((bhd_directory, bhd_directory_range, first_payload_range_beginning))
}

fn transfer_from_flash_to_io<T: FlashRead + FlashWrite>(
    storage: &T,
    mut off: Location,
    mut size: usize,
    destination: &mut impl std::io::Write,
) {
    let mut buffer = [0u8; 8192];
    while size > 0 {
        let chunk_size = min(buffer.len(), size);
        storage.read_exact(off, &mut buffer[..chunk_size]).unwrap();
        destination.write_all(&buffer[..chunk_size]).unwrap();
        size -= chunk_size;
        off += chunk_size as u32;
    }
}

fn create_dumpfile(
    existing_filenames: &mut HashSet<PathBuf>,
    blob_dump_dirname: &PathBuf,
    section: &str,
    typ_string: String,
    instance: u8,
    sub_program: u8,
) -> (File, PathBuf) {
    let mut path = PathBuf::new();
    path.push(blob_dump_dirname);
    path.push(section);
    let basename = Path::new(&typ_string);
    path.push(format!(
        "{}-i{:02x}-s{:02x}.bin",
        basename.display(),
        instance,
        sub_program,
    ));
    if existing_filenames.contains(&path) {
        panic!(
            "Refusing to create two files with the same name: {}",
            path.display()
        );
    }
    existing_filenames.insert(path.clone());
    (File::create(&path).expect("creation failed"), path)
}

fn dump_psp_directory<T: FlashRead + FlashWrite>(
    storage: &T,
    psp_directory: &PspDirectory,
    blob_dump_dirname: &Option<PathBuf>,
) -> SerdePspDirectoryVariant {
    if let Some(blob_dump_dirname) = &blob_dump_dirname {
        let mut path = PathBuf::new();
        path.push(blob_dump_dirname);
        path.push("psp-default");
        fs::create_dir_all(path).unwrap();
    }
    // TODO: Handle the other variant (PspComboDirectory)
    let mut blob_dump_filenames = HashSet::<PathBuf>::new();
    SerdePspDirectoryVariant::PspDirectory(SerdePspDirectory {
        entries: psp_directory.entries().map_while(|e| {
        if let Ok(typ) = e.typ_or_err() {
            match typ {
                        PspDirectoryEntryType::SecondLevelDirectory => {
                    let payload_beginning =
                        psp_directory.payload_beginning(&e).unwrap();
                    let size = e.size().unwrap() as usize;
                            let mut dir: [u8; 10000] = [0u8; 10000]; // FIXME size
                            storage
                                .read_exact(
                                    payload_beginning,
                                    &mut dir[0..size],
                                )
                                .unwrap();

                            // FIXME convert dir to SerdePspDirectory
                            let sub_psp_directory = PspDirectory::load(storage, payload_beginning, /*FIXME mode3 base*/0, /*FIXME mmio*/None).unwrap();
                            let subdir = match blob_dump_dirname {
                                None => None,
                                Some(x) => {
                                    let mut t = x.clone();
                                    t.push("psp-second-level");
                                    Some(t)
                                }
                            };
                            let variant = dump_psp_directory(storage, &sub_psp_directory, &subdir);
                            Some(SerdePspEntry {
                                source: SerdePspEntrySource::SecondLevelDirectory(match variant {
                                    SerdePspDirectoryVariant::PspDirectory(d) => d,
                                    _ => {
                                        panic!("???");
                                    }
                                }),
                                target: serde_from_psp_entry(
                                    psp_directory,
                                    &e,
                                ),
                            })
                        }
            _ => {
            let blob_export = match psp_directory.payload_beginning(&e) {
               Ok(beginning) => {
                   let typ_string = typ.to_string();
                   let size = e.size().unwrap() as usize;
                   if let Some(blob_dump_dirname) = blob_dump_dirname {
                       let (data_file, path) = create_dumpfile(&mut blob_dump_filenames, blob_dump_dirname, "psp-default", typ_string, e.instance(), e.sub_program());
                       Some((Some(data_file), path, beginning, size))
                       //let (data_file, path) = create_dumpfile(&mut blob_dump_filenames, blob_dump_dirname, "psp-default", typ_string, e.instance(), e.sub_program());
                       //Some((data_file, path, beginning, size))
                   } else {
                       Some((None, Path::new("????").to_path_buf(), beginning, size))
                   }
               }
               Err(amd_efs::Error::DirectoryTypeMismatch) => {
                   None
               }
               Err(e) => {
                   panic!("not handled yet (implementation limitation) {:?}", e);
               }
            };

            Some(SerdePspEntry {
                source: match blob_export {
                    Some((_, ref path, _beginning, _size)) => {
                        SerdePspEntrySource::BlobFile(path.into())
                    }
                    None => { /* probably a value or something */
                        SerdePspEntrySource::Value(1) // FIXME: Value
                    }
                },
                target: SerdePspDirectoryEntry {
                    attrs: SerdePspDirectoryEntryAttrs {
                        type_: typ,
                        sub_program: e.sub_program_or_err().unwrap(),
                        rom_id: e.rom_id_or_err().unwrap(),
                        instance: e.instance(),
                    },
                    blob: match blob_export {
                        None => {
                           None
                        }
                        Some((Some(mut data_file), ref _path, beginning, size)) => {
                                transfer_from_flash_to_io(
                                    storage,
                                    beginning,
                                    size,
                                    &mut data_file,
                                );
                            Some(SerdePspDirectoryEntryBlob {
                        flash_location: Some(psp_directory.payload_beginning(&e).unwrap()),
                        size: Some(size.try_into().unwrap()),
                    })
                    }
                    Some((None, _, _, _)) => {
                            Some(SerdePspDirectoryEntryBlob {
                        flash_location: Some(psp_directory.payload_beginning(&e).unwrap()),
                        size: Some(e.size().unwrap()), // FIXME what if it doesn't apply?
                    })
                    }
                }
                },
            })}}
        } else {
            eprintln!("WARNING: PSP entry with unknown type was skipped {:?}", e);
            None
        }
        }).collect()
    })
}

fn serde_from_bhd_entry(
    directory: &BhdDirectory,
    entry: &BhdDirectoryEntry,
) -> SerdeBhdDirectoryEntry {
    SerdeBhdDirectoryEntry {
        attrs: SerdeBhdDirectoryEntryAttrs {
            type_: entry.typ_or_err().unwrap(),
            region_type: entry.region_type_or_err().unwrap(),
            reset_image: entry.reset_image_or_err().unwrap(),
            copy_image: entry.copy_image_or_err().unwrap(),
            read_only: entry.read_only_or_err().unwrap(),
            compressed: entry.compressed_or_err().unwrap(),
            instance: entry.instance_or_err().unwrap(),
            sub_program: entry.sub_program_or_err().unwrap(),
            rom_id: entry.rom_id_or_err().unwrap(),
        },
        blob: Some(SerdeBhdDirectoryEntryBlob {
            flash_location: Some(directory.payload_beginning(&entry).unwrap()),
            size: entry.size(),
            ram_destination_address: entry.destination_location(), // FIXME: rename amd-efs destination location to ram_destination_address
        }),
    }
}

fn serde_from_psp_entry(
    directory: &PspDirectory,
    entry: &PspDirectoryEntry,
) -> SerdePspDirectoryEntry {
    SerdePspDirectoryEntry {
        attrs: SerdePspDirectoryEntryAttrs {
            type_: entry.typ_or_err().unwrap(),
            instance: entry.instance_or_err().unwrap(),
            sub_program: entry.sub_program_or_err().unwrap(),
            rom_id: entry.rom_id_or_err().unwrap(),
        },
        blob: Some(SerdePspDirectoryEntryBlob {
            flash_location: Some(directory.payload_beginning(&entry).unwrap()),
            size: entry.size(),
        }),
    }
}

fn dump_bhd_directory<'a, T: FlashRead + FlashWrite>(
    storage: &T,
    bhd_directory: &BhdDirectory,
    apcb_buffer_option: &mut Option<&'a mut [u8]>,
    blob_dump_dirname: &Option<PathBuf>,
) -> SerdeBhdDirectoryVariant<'a> {
    if let Some(blob_dump_dirname) = &blob_dump_dirname {
        let mut path = PathBuf::new();
        path.push(blob_dump_dirname);
        path.push("bhd-default");
        fs::create_dir_all(path).unwrap();
    }
    let mut blob_dump_filenames = HashSet::<PathBuf>::new();
    SerdeBhdDirectoryVariant::BhdDirectory(SerdeBhdDirectory {
        entries: bhd_directory
            .entries()
            .map_while(|entry| {
                let entry = entry.clone();
                if let Ok(typ) = entry.typ_or_err() {
                    let payload_beginning =
                        bhd_directory.payload_beginning(&entry).unwrap();
                    let size = entry.size().unwrap() as usize;
                    match typ {

                        BhdDirectoryEntryType::ApcbBackup
                        | BhdDirectoryEntryType::Apcb
                            if apcb_buffer_option.is_some() =>
                        {
                            let apcb_buffer = apcb_buffer_option
                                .take()
                                .expect("only one APCB");
                            storage
                                .read_exact(
                                    payload_beginning,
                                    &mut apcb_buffer[0..size],
                                )
                                .unwrap();

                            let apcb = Apcb::load(
                                std::borrow::Cow::Borrowed(
                                    &mut apcb_buffer[..],
                                ),
                                &ApcbIoOptions::default(),
                            )
                            .unwrap();
                            apcb.validate(None).unwrap(); // TODO: abl0 version ?
                            Some(SerdeBhdEntry {
                                source: SerdeBhdSource::ApcbJson(apcb),
                                target: serde_from_bhd_entry(
                                    &bhd_directory,
                                    &entry,
                                ),
                            })
                        }

                        BhdDirectoryEntryType::SecondLevelDirectory => {
                            let mut dir: [u8; 10000] = [0u8; 10000]; // FIXME size
                            storage
                                .read_exact(
                                    payload_beginning,
                                    &mut dir[0..size],
                                )
                                .unwrap();

                            // FIXME convert dir to SerdeBhdDirectory
                            let sub_bhd_directory = BhdDirectory::load(storage, payload_beginning, /*FIXME mode3 base*/0, /*FIXME mmio*/None).unwrap();
                            let subdir = match blob_dump_dirname {
                                None => None,
                                Some(x) => {
                                    let mut t = x.clone();
                                    t.push("bhd-second-level");
                                    Some(t)
                                }
                            };
                            let variant = dump_bhd_directory(storage, &sub_bhd_directory, apcb_buffer_option, &subdir); //SerdeBhdDirectoryVariant
                            Some(SerdeBhdEntry {
                                source: SerdeBhdSource::SecondLevelDirectory(match variant {
                                    SerdeBhdDirectoryVariant::BhdDirectory(d) => d,
                                    _ => {
                                        panic!("???");
                                    }
                                }),
                                target: serde_from_bhd_entry(
                                    bhd_directory,
                                    &entry,
                                ),
                            })
                        }
                        BhdDirectoryEntryType::Apob => Some(SerdeBhdEntry {
                            source: SerdeBhdSource::Implied,
                            target: serde_from_bhd_entry(
                                    bhd_directory,
                                    &entry,
                            )
                        }),
                        typ => Some(SerdeBhdEntry {
                            source: if let Some(blob_dump_dirname) =
                                &blob_dump_dirname
                            {
                                let typ_string = typ.to_string();
                                let (mut data_file, path) = create_dumpfile(
                                    &mut blob_dump_filenames,
                                    blob_dump_dirname,
                                    "bhd-default",
                                    typ_string,
                                    entry.instance(),
                                    entry.sub_program(),
                                );
                                transfer_from_flash_to_io(
                                    storage,
                                    payload_beginning,
                                    size,
                                    &mut data_file,
                                );
                                SerdeBhdSource::BlobFile(path)
                            } else {
                                SerdeBhdSource::Implied
                            },
                            target: serde_from_bhd_entry(
                                bhd_directory,
                                &entry,
                            ),
                        }),
                    }
                } else {
                    eprintln!(
                        "WARNING: BHD entry with unknown type was skipped {:?}",
                        entry
                    );
                    None
                }
            })
            .collect(),
    })
}

fn dump(
    image_filename: &Path,
    blob_dump_dirname: Option<PathBuf>,
) -> std::io::Result<()> {
    let filename = image_filename;
    let storage = FlashImage::load(filename)?;
    let filesize = storage.file_size()?;
    let amd_physical_mode_mmio_size =
        if filesize <= 0x100_0000 { Some(filesize as u32) } else { None };
    let efs = Efs::load(&storage, None, amd_physical_mode_mmio_size).unwrap();
    let gen = [
        ProcessorGeneration::Turin,
        ProcessorGeneration::Genoa,
        ProcessorGeneration::Milan,
    ]
    .iter()
    .find(|&gen| efs.compatible_with_processor_generation(*gen))
    .expect("only Milan, Genoa and Turin are supported for dumping right now");
    let mut apcb_buffer = [0xFFu8; Apcb::MAX_SIZE];
    let mut apcb_buffer_option = Some(&mut apcb_buffer[..]);
    let psp_main_directory_flash_location =
        Some(efs.psp_directory().unwrap().beginning());
    let bhd_main_directory_flash_location =
        Some(efs.bhd_directory(None).unwrap().beginning());
    let config = SerdeConfig {
        processor_generation: *gen,
        spi_mode_bulldozer: efs.spi_mode_bulldozer().unwrap(),
        spi_mode_zen_naples: efs.spi_mode_zen_naples().unwrap(),
        spi_mode_zen_rome: efs.spi_mode_zen_rome().unwrap(),
        espi0_configuration: efs.espi0_configuration().unwrap(),
        espi1_configuration: efs.espi1_configuration().unwrap(),
        psp_main_directory_flash_location,
        bhd_main_directory_flash_location,
        // TODO: psp_directory or psp_combo_directory
        psp: dump_psp_directory(
            &storage,
            &efs.psp_directory().unwrap(),
            &blob_dump_dirname,
        ),
        // TODO: bhd_directory or bhd_combo_directory
        bhd: dump_bhd_directory(
            &storage,
            &efs.bhd_directory(None).unwrap(),
            &mut apcb_buffer_option,
            &blob_dump_dirname,
        ),
    };
    if let Some(blob_dump_dirname) = &blob_dump_dirname {
        let mut path = PathBuf::new();
        path.push(blob_dump_dirname);
        path.push("config.efs.json5");
        use std::io::Write;
        let mut file = File::create(&path).expect("creation failed");
        writeln!(file, "{}", serde_json::to_string_pretty(&config).unwrap())?;
    } else {
        println!("{}", serde_json::to_string_pretty(&config)?);
    }
    Ok(())
}

fn prepare_psp_directory_contents(
    serde_psp_directory: SerdePspDirectory,
    resolve_blob: impl Fn(
        PathBuf,
    ) -> std::prelude::v1::Result<PathBuf, std::io::Error>,
    efs_configuration_filename: &Path,
) -> (
    Option<u32>,
    Option<(u8, u8, u8, u8)>,
    AddressMode,
    Option<(SerdePspDirectory, Option<SerdePspDirectoryEntryBlob>)>,
    Vec<(PspDirectoryEntry, Option<Location>, Option<Vec<u8>>)>,
) {
    // ================================ PSP =============================

    let mut abl_version: Option<u32> = None;
    let mut abl_version_found = false;
    let mut smu_version: Option<(u8, u8, u8, u8)> = None;
    let mut smu_version_found = false;
    let psp_directory_address_mode = AddressMode::EfsRelativeOffset;
    let mut psp_second_level_directory_template: Option<(
        SerdePspDirectory,
        Option<SerdePspDirectoryEntryBlob>,
    )> = None;
    let psp_raw_entries = serde_psp_directory.entries.into_iter().flat_map(|entry| {
                let mut raw_entry = PspDirectoryEntry::try_from_with_context(
                    psp_directory_address_mode,
                    &entry.target
                ).unwrap();
                //eprintln!("{:?}", entry.target.attrs);
                let blob_slot_settings = &entry.target.blob;
                // blob_slot_settings is optional.
                // Value means no blob slot settings allowed

                match entry.source {
                    SerdePspEntrySource::Value(x) => {
                        // FIXME: assert!(blob_slot_settings.is_none()); fails for some reason
                        // DirectoryRelativeOffset is the one that can always be overridden
                        raw_entry.set_source(AddressMode::DirectoryRelativeOffset, ValueOrLocation::Value(x)).unwrap();
                        vec![(raw_entry, None, None)]
                    }
                    SerdePspEntrySource::BlobFile(
                        blob_filename,
                    ) => {
                        let flash_location =
                            blob_slot_settings.as_ref().and_then(|x| x.flash_location);
                        let x: Option<Location> = flash_location;
                        let blob_filename = resolve_blob(blob_filename.to_path_buf()).unwrap();
                        let body = std::fs::read(&blob_filename).unwrap();
                        raw_entry.set_size(Some(body.len().try_into().unwrap()));

                        match raw_entry.typ_or_err() {
                            Ok(PspDirectoryEntryType::Abl0) |
                            Ok(PspDirectoryEntryType::Abl1) |
                            Ok(PspDirectoryEntryType::Abl2) |
                            Ok(PspDirectoryEntryType::Abl3) |
                            Ok(PspDirectoryEntryType::Abl4) |
                            Ok(PspDirectoryEntryType::Abl5) |
                            Ok(PspDirectoryEntryType::Abl6) |
                            Ok(PspDirectoryEntryType::Abl7) => {
                                let new_abl_version = abl_file_version(&blob_filename);
                                if !abl_version_found {
                                    abl_version = new_abl_version;
                                    abl_version_found = true
                                }
                                // For now, we do not support different ABL versions in the same image.
                                if new_abl_version != abl_version {
                                    panic!("different ABL versions in the same flash are unsupported")
                                }
                            }
                            Ok(PspDirectoryEntryType::SmuOffChipFirmware8) | Ok(PspDirectoryEntryType::SmuOffChipFirmware12) => {
                                let new_smu_version = smu_file_version(&blob_filename);
                                if !smu_version_found {
                                    smu_version = new_smu_version;
                                    smu_version_found = true
                                }
                                // For now, we do not support different SMU firmware versions in the same image.
    /* FIXME
                                if new_smu_version != smu_version {
                                    panic!("different SMU versions in the same flash are unsupported")
                                }
*/
                            }
                            _ => {
                            }
                        }
                        vec![(raw_entry, x, Some(body))]
                    }
                    SerdePspEntrySource::SecondLevelDirectory(d) => {
                        // It is impossible to just create an active PspDirectory here since:
                        // - Allocation of its location has not been done yet
                        // - So we don't know where the directory is going to be.
                        // - But there can be entries in that new directory that are directory-relative.
                        assert!(psp_second_level_directory_template.is_none());
                        psp_second_level_directory_template = Some((d, entry.target.blob));
                        vec![]
                    }
                }
            })
            .collect::<Vec<(PspDirectoryEntry, Option<Location>, Option<Vec<u8>>)>>()
    ;
    (
        abl_version,
        smu_version,
        psp_directory_address_mode,
        psp_second_level_directory_template,
        psp_raw_entries,
    )
}

fn prepare_bhd_directory_contents<'a>(
    serde_bhd_directory: SerdeBhdDirectory<'a>,
    resolve_blob: impl Fn(
        PathBuf,
    ) -> std::prelude::v1::Result<PathBuf, std::io::Error>,
    efs_configuration_filename: &Path,
) -> (
    AddressMode,
    bool,
    Option<(SerdeBhdDirectory<'a>, Option<SerdeBhdDirectoryEntryBlob>)>,
    Vec<(BhdDirectoryEntry, Option<Location>, Option<Vec<u8>>)>,
    bool,
) {
    let mut custom_bios_reset_entry: bool = false;
    let apcb_to_io_error = |e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "APCB error: {e:?} in file {:?}",
                efs_configuration_filename
            ),
        )
    };
    let bhd_directory_address_mode = AddressMode::EfsRelativeOffset;
    let mut custom_apob = false;
    let mut bhd_second_level_directory_template: Option<(
        SerdeBhdDirectory<'_>,
        Option<SerdeBhdDirectoryEntryBlob>,
    )> = None;
    let bhd_raw_entries = serde_bhd_directory
        .entries
        .into_iter()
        .flat_map(|entry| {
            let mut raw_entry = BhdDirectoryEntry::try_from_with_context(
                bhd_directory_address_mode,
                &entry.target,
            )
            .unwrap();
            if let Ok(BhdDirectoryEntryType::Bios) = raw_entry.typ_or_err() {
                if raw_entry.reset_image() {
                    assert!(!custom_bios_reset_entry);
                    custom_bios_reset_entry = true;
                }
            }
            let blob_slot_settings = entry.target.blob;
            let mut flash_location =
                blob_slot_settings.as_ref().and_then(|x| x.flash_location);
            let x: Option<Location> = flash_location;

            // done by try_from: raw_entry.set_destination_location(ram_destination_address);
            // done by try_from: raw_entry.set_size(size);
            match entry.source {
                SerdeBhdSource::Implied => {
                    assert!(
                        entry.target.attrs.type_ == BhdDirectoryEntryType::Apob
                    );
                    match x {
                        Some(0) => {
                            /* This APOB entry is supposed to (really) have this target, so that's what was dumped: {
                                type: 'Apob',
                                region_type: 'Normal',
                                reset_image: false,
                                copy_image: false,
                                read_only: false,
                                compressed: false,
                                instance: 0,
                                sub_program: 0,
                                rom_id: 'SpiCs1',
                                flash_location: 0,
                                size: 0,
                                ram_destination_address: user-defined
                            }

                            Obviously, that flash_location should not actually be allocated.
                            */
                            flash_location = None;
                        }
                        None => { // ok
                        }
                        _ => assert!(false),
                    }
                    custom_apob = true;
                    raw_entry.set_size(Some(0));
                    vec![(raw_entry, None, None)]
                }
                SerdeBhdSource::BlobFile(blob_filename) => {
                    assert!(
                        entry.target.attrs.type_ != BhdDirectoryEntryType::Apob
                    );
                    let blob_filename = resolve_blob(blob_filename).unwrap();
                    let body = std::fs::read(blob_filename).unwrap();
                    raw_entry.set_size(Some(body.len().try_into().unwrap()));
                    vec![(raw_entry, x, Some(body))]
                }
                SerdeBhdSource::ApcbJson(apcb) => {
                    // Note: We need to do this
                    // manually because validation
                    // needs ABL_VERSION.
                    apcb.validate(None /*abl_version*/)
                        .map_err(apcb_to_io_error)
                        .unwrap();
                    let buf =
                        apcb.save_no_inc().map_err(apcb_to_io_error).unwrap();
                    let bufref = buf.as_ref();
                    if raw_entry.size().is_none() {
                        raw_entry
                            .set_size(Some(bufref.len().try_into().unwrap()));
                    };

                    vec![(raw_entry, None, Some(buf.into_owned()))]
                }
                SerdeBhdSource::SecondLevelDirectory(d) => {
                    // It is impossible to just create an active BhdDirectory here since:
                    // - Allocation of its location has not been done yet
                    // - So we don't know where the directory is going to be.
                    // - But there can be entries in that new directory that are directory-relative.
                    assert!(bhd_second_level_directory_template.is_none());
                    bhd_second_level_directory_template =
                        Some((d, blob_slot_settings));
                    vec![]
                }
            }
        })
        .collect::<Vec<(BhdDirectoryEntry, Option<Location>, Option<Vec<u8>>)>>(
        );
    (
        bhd_directory_address_mode,
        custom_apob,
        bhd_second_level_directory_template,
        bhd_raw_entries,
        custom_bios_reset_entry,
    )
}

fn generate(
    output_filename: &Path,
    image_size: u32,
    efs_configuration_filename: &Path,
    reset_image_filename: &Option<PathBuf>,
    blobdirs: Vec<PathBuf>,
    verbose: bool,
) -> std::io::Result<()> {
    let filename = &output_filename;
    let flash_to_io_error = |e: amd_flash::Error| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Flash error: {e:?} in file {filename:?}"),
        )
    };
    let efs_to_io_error = |e: amd_efs::Error| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Config error: {e:?} in file {:?}",
                efs_configuration_filename
            ),
        )
    };
    let json5_to_io_error = |e: json5::Error| match e {
        json5::Error::Message { ref msg, ref location } => std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "JSON5 error: {msg} in file {:?} at {}",
                efs_configuration_filename,
                match location {
                    None => "unknown location".to_owned(),
                    Some(x) => format!("{x:?}"),
                }
            ),
        ),
    };
    let amd_host_image_builder_config_error_to_io_error =
        |e: amd_host_image_builder_config::Error| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Config error: {e:?} in file {:?}",
                    reset_image_filename
                ),
            )
        };
    let blobdirs = &blobdirs;
    let resolve_blob = |blob_filename: PathBuf| -> std::io::Result<PathBuf> {
        if blob_filename.has_root() {
            if blob_filename.exists() {
                Ok(blob_filename)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Blob read error: Could not find file {blob_filename:?}",
                    ),
                ))
            }
        } else {
            for blobdir in blobdirs {
                let fullname = blobdir.join(&blob_filename);
                if fullname.exists() {
                    if verbose {
                        eprintln!("Info: Using blob {fullname:?}");
                    }
                    return Ok(fullname);
                }
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Blob read error: Could not find file {blob_filename:?} \
(neither directly nor in any of the directories {blobdirs:?})",
                ),
            ))
        }
    };

    // Needs to be at least 0x1000.
    // See also DirectoryAdditionalInfo::with_max_size_checked.
    const ERASABLE_BLOCK_SIZE: usize = 0x1000;
    const_assert!(ERASABLE_BLOCK_SIZE.is_power_of_two());
    let storage =
        FlashImage::create(filename, image_size, ERASABLE_BLOCK_SIZE)?;
    storage.erase()?;
    let path = Path::new(&efs_configuration_filename);
    let data = std::fs::read_to_string(path)?;
    let config: SerdeConfig =
        json5::from_str(&data).map_err(json5_to_io_error)?;

    let SerdeConfig {
        processor_generation,
        spi_mode_bulldozer,
        spi_mode_zen_naples,
        spi_mode_zen_rome,
        espi0_configuration,
        espi1_configuration,
        psp_main_directory_flash_location,
        bhd_main_directory_flash_location,
        psp,
        bhd,
    } = config;
    let host_processor_generation = processor_generation;
    let mut allocator = ArenaFlashAllocator::new(
        crate::static_config::EFH_BEGINNING(host_processor_generation),
        crate::static_config::EFH_SIZE,
        ErasableRange::new(
            storage.erasable_location(0).unwrap(),
            storage.erasable_location(image_size).unwrap(),
        ),
    )
    .map_err(flash_to_io_error)?;
    // Avoid area around 0 because AMD likes to use Efh locations == 0 to
    // mean "invalid".  We reserve the lowest sector (64 KiB) for Hubris's use,
    // particularly to store which host BSU is active.
    let _invalid = allocator.take_at_least(0x1_0000);

    let mut efs = match Efs::create(
        &storage,
        host_processor_generation,
        static_config::EFH_BEGINNING(host_processor_generation),
        Some(image_size),
    ) {
        Ok(efs) => efs,
        Err(e) => {
            eprintln!("Error on creation: {e:?}");
            std::process::exit(1);
        }
    };
    efs.set_spi_mode_bulldozer(spi_mode_bulldozer);
    efs.set_spi_mode_zen_naples(spi_mode_zen_naples);
    efs.set_spi_mode_zen_rome(spi_mode_zen_rome);
    efs.set_espi0_configuration(espi0_configuration);
    efs.set_espi1_configuration(espi1_configuration);

    let (
        abl_version,
        smu_version,
        psp_directory_address_mode,
        psp_second_level_directory_template,
        mut psp_raw_entries,
    ) = prepare_psp_directory_contents(
        match psp {
            SerdePspDirectoryVariant::PspDirectory(serde_psp_directory) => {
                serde_psp_directory
            }
            _ => {
                todo!();
            }
        },
        resolve_blob,
        efs_configuration_filename,
    );
    // Since we need to store the pointer to the second-level directory
    // inside the first-level directory, do the second-level directory first.

    let (
        mut psp_main_directory,
        psp_main_directory_range,
        psp_main_first_payload_range_beginning,
    ) = if let Some((
        psp_second_level_directory_template,
        psp_second_level_directory_container_blob,
    )) = psp_second_level_directory_template
    {
        let (
            psp_second_level_abl_version,
            psp_second_level_smu_version,
            psp_second_level_directory_address_mode,
            psp_third_level_directory_template,
            mut psp_second_level_raw_entries,
        ) = prepare_psp_directory_contents(
            psp_second_level_directory_template,
            resolve_blob,
            efs_configuration_filename,
        );
        assert!(psp_third_level_directory_template.is_none());
        // FIXME assert!(psp_second_level_custom_apob);
        let (
            mut psp_second_level_directory,
            psp_second_level_directory_range,
            psp_second_level_first_payload_range_beginning,
        ) = create_psp_directory(
            PspDirectoryHeader::SECOND_LEVEL_COOKIE,
            psp_second_level_directory_container_blob
                .and_then(|e| e.flash_location),
            &mut psp_second_level_raw_entries,
            psp_second_level_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename, //
        )?;
        let psp_second_level_directory_range_beginning =
            Location::from(psp_second_level_directory_range.beginning);
        let psp_second_level_directory_blob = psp_second_level_directory
            .save1(
                storage.erasable_block_size(),
                &psp_second_level_directory_range,
                psp_second_level_first_payload_range_beginning.unwrap(),
            )
            .map_err(efs_to_io_error)?;

        // Add entry for the subdirectory to the directory; TODO maybe reuse psp_second_level_directory_container_blob try_with_context more?
        let mut psp_second_level_raw_entry = PspDirectoryEntry::new()
            .with_typ(PspDirectoryEntryType::SecondLevelDirectory)
            .with_sub_program(0)
            .with_instance(0)
            //.with_rom_id(PspDirectoryRomId::SpiCs1)
            .build();
        psp_second_level_raw_entry
            .set_size(Some(psp_second_level_directory_blob.len() as u32));
        // Note: Insert at 0 is very bad (hangs BMC ability to shut down host)
        psp_raw_entries.push((
            psp_second_level_raw_entry,
            Some(psp_second_level_directory_range_beginning),
            Some(psp_second_level_directory_blob),
        ));

        let result = create_psp_directory(
            PspDirectoryHeader::FIRST_LEVEL_COOKIE,
            psp_main_directory_flash_location,
            &mut psp_raw_entries,
            psp_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename,
        )?;
        psp_raw_entries.append(&mut psp_second_level_raw_entries);
        result
    } else {
        create_psp_directory(
            PspDirectoryHeader::FIRST_LEVEL_COOKIE,
            psp_main_directory_flash_location,
            &mut psp_raw_entries,
            psp_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename,
        )?
    };
    let psp_main_directory_blob = psp_main_directory
        .save1(
            storage.erasable_block_size(),
            &psp_main_directory_range,
            psp_main_first_payload_range_beginning.unwrap(),
        )
        .map_err(efs_to_io_error)?;
    storage
        .erase_and_write_blocks(
            psp_main_directory_range.beginning,
            &psp_main_directory_blob,
        )
        .map_err(flash_to_io_error)?;
    efs.set_main_psp_directory(&psp_main_directory).map_err(efs_to_io_error)?;

    if verbose {
        // See AgesaBLReleaseNotes.txt, section "ABL Version String"
        match abl_version {
            Some(v) => println!("Info: ABL version: 0x{v:x}"),
            None => println!("Info: ABL version unknown"),
        }
        // See SmuReleaseNotesGn.txt, text "Version"
        match smu_version {
            Some((0, s1, s2, s3)) => {
                println!("Info: SMU firmware version: {s1}.{s2}.{s3}")
            }
            Some((s0, s1, s2, s3)) => {
                println!("Info: SMU firmware version: {s0}.{s1}.{s2}.{s3}")
            }
            None => println!("Info: SMU firmware version unknown"),
        }
    }

    // ================================ BHD =============================

    let (
        bhd_directory_address_mode,
        custom_apob,
        bhd_second_level_directory_template,
        mut bhd_raw_entries,
        custom_bios_reset_entry,
    ) = match bhd {
        SerdeBhdDirectoryVariant::BhdDirectory(serde_bhd_directory) => {
            prepare_bhd_directory_contents(
                serde_bhd_directory,
                resolve_blob,
                efs_configuration_filename,
            )
        }
        _ => {
            todo!();
        }
    };

    if !custom_apob {
        let apob_entry = BhdDirectoryEntry::new_payload(
            AddressMode::PhysicalAddress,
            BhdDirectoryEntryType::Apob,
            Some(0),
            Some(ValueOrLocation::PhysicalAddress(0)),
            Some(0x400_0000),
        )
        .unwrap();
        bhd_raw_entries.push((apob_entry, None, None));
    }

    let reset_image_entry = if let Some(reset_image_filename) =
        reset_image_filename
    {
        if custom_bios_reset_entry {
            panic!("It's impossible to use both a Bios type Reset entry in the config file and a (Bios) Reset image on the command line");
        }
        let (reset_image_entry, reset_image_body) =
            bhd_directory_add_reset_image(&reset_image_filename)
                .map_err(amd_host_image_builder_config_error_to_io_error)?;
        bhd_raw_entries.push((
            reset_image_entry,
            None,
            Some(reset_image_body.clone()),
        ));
        Some((reset_image_entry, reset_image_body))
    } else {
        if !custom_bios_reset_entry {
            panic!("Without a Bios Reset entry, the target will not boot. Hint: Please Specify '-r', or add an entry to the config file with type 'Bios'");
        }
        None
    };

    // Since we need to store the pointer to the second-level directory
    // inside the first-level directory, do the second-level directory first.

    let (
        mut bhd_main_directory,
        bhd_main_directory_range,
        bhd_main_first_payload_range_beginning,
    ) = if let Some((
        bhd_second_level_directory_template,
        bhd_second_level_directory_container_blob,
    )) = bhd_second_level_directory_template
    {
        let (
            bhd_second_level_directory_address_mode,
            bhd_second_level_custom_apob,
            bhd_third_level_directory_template,
            mut bhd_second_level_raw_entries,
            bhd_second_level_custom_bios_reset_entry,
        ) = prepare_bhd_directory_contents(
            bhd_second_level_directory_template,
            resolve_blob,
            efs_configuration_filename,
        );
        assert!(
            bhd_second_level_custom_bios_reset_entry == custom_bios_reset_entry
        );

        if let Some((reset_image_entry, reset_image_body)) = reset_image_entry {
            bhd_second_level_raw_entries.push((
                reset_image_entry,
                None,
                Some(reset_image_body),
            ));
        }

        assert!(bhd_third_level_directory_template.is_none());
        // FIXME assert!(bhd_second_level_custom_apob);
        let (
            mut bhd_second_level_directory,
            bhd_second_level_directory_range,
            bhd_second_level_first_payload_range_beginning,
        ) = create_bhd_directory(
            BhdDirectoryHeader::SECOND_LEVEL_COOKIE,
            bhd_second_level_directory_container_blob
                .and_then(|e| e.flash_location),
            &mut bhd_second_level_raw_entries,
            bhd_second_level_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename, //
        )?;
        let bhd_second_level_directory_range_beginning =
            Location::from(bhd_second_level_directory_range.beginning);
        let bhd_second_level_directory_blob = bhd_second_level_directory
            .save1(
                storage.erasable_block_size(),
                &bhd_second_level_directory_range,
                bhd_second_level_first_payload_range_beginning.unwrap(),
            )
            .map_err(efs_to_io_error)?;

        // Add entry for the subdirectory to the directory
        let mut raw_entry = BhdDirectoryEntry::new()
            .with_typ(BhdDirectoryEntryType::SecondLevelDirectory)
            .with_sub_program(0)
            .with_instance(0)
            //.with_rom_id(PspDirectoryRomId::SpiCs1)
            .build();
        raw_entry.set_size(Some(bhd_second_level_directory_blob.len() as u32));
        bhd_raw_entries.push((
            raw_entry,
            Some(bhd_second_level_directory_range_beginning),
            Some(bhd_second_level_directory_blob),
        ));

        let result = create_bhd_directory(
            BhdDirectoryHeader::FIRST_LEVEL_COOKIE,
            bhd_main_directory_flash_location,
            &mut bhd_raw_entries,
            bhd_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename,
        )?;
        bhd_raw_entries.append(&mut bhd_second_level_raw_entries);
        result
    } else {
        create_bhd_directory(
            BhdDirectoryHeader::FIRST_LEVEL_COOKIE,
            bhd_main_directory_flash_location,
            &mut bhd_raw_entries,
            bhd_directory_address_mode,
            &storage,
            &mut allocator,
            &mut efs,
            &output_filename,
        )?
    };
    let bhd_main_directory_blob = bhd_main_directory
        .save1(
            storage.erasable_block_size(),
            &bhd_main_directory_range,
            bhd_main_first_payload_range_beginning.unwrap(),
        )
        .map_err(efs_to_io_error)?;
    storage
        .erase_and_write_blocks(
            bhd_main_directory_range.beginning,
            &bhd_main_directory_blob,
        )
        .map_err(flash_to_io_error)?;
    efs.set_main_bhd_directory(&bhd_main_directory).map_err(efs_to_io_error)?;

    // ============================== Payloads =========================

    for (raw_entry, _, blob_body) in psp_raw_entries {
        eprintln!("PSP entry {:?}", raw_entry);
        if let Some(blob_body) = blob_body {
            let source =
                match raw_entry.source(psp_directory_address_mode).unwrap() {
                    ValueOrLocation::EfsRelativeOffset(x) => {
                        eprintln!("X {}", x);
                        storage.erasable_location(x).unwrap()
                    }
                    _ => {
                        todo!()
                    }
                };
            storage
                .erase_and_write_blocks(source, &blob_body)
                .map_err(flash_to_io_error)?;
        }
    }

    for (raw_entry, _, blob_body) in bhd_raw_entries {
        //eprintln!("BHD entry {:?}", raw_entry);
        if let Some(blob_body) = blob_body {
            let source = match raw_entry
                .source(bhd_directory_address_mode)
                .unwrap()
            {
                ValueOrLocation::EfsRelativeOffset(x) => {
                    match storage.erasable_location(x) {
                        Some(y) => y,
                        None => {
                            panic!("BHD entry {:?}: EfsRelativeOffset is {} which is not on an erase boundary", raw_entry, x);
                        }
                    }
                }
                x => {
                    eprintln!("{x:?}");
                    todo!()
                }
            };
            storage
                .erase_and_write_blocks(source, &blob_body)
                .map_err(flash_to_io_error)?;
        }
    }

    Ok(())
}

fn run() -> std::io::Result<()> {
    let compat_args = std::env::args().collect::<Vec<String>>();
    // Older versions of amd-host-image-builder didn't have subcommands since
    // it would only have one functionality: To generate images.
    // Support the old command line syntax as well.
    let opts = if compat_args.len() > 1 && compat_args[1].starts_with("-") {
        let mut compat_args = compat_args.clone();
        compat_args.insert(1, "generate".to_string());
        Opts::from_iter(&compat_args)
    } else {
        Opts::from_args()
    };
    match opts {
        Opts::Dump { input_filename, blob_dump_dirname } => {
            dump(&input_filename, blob_dump_dirname)
        }
        Opts::Generate {
            output_filename,
            output_size,
            efs_configuration_filename,
            reset_image_filename,
            blobdirs,
            verbose,
        } => generate(
            &output_filename,
            match u32::try_from(output_size.as_u64()).expect("Size <= 32 MiB") {
                0x100_0000 => 0x100_0000,
                0x200_0000 => 0x200_0000,
                _ => {
                    return Err(std::io::Error::other(format!(
                        "unsupported output size {}",
                        output_size
                    )));
                }
            },
            &efs_configuration_filename,
            &reset_image_filename,
            blobdirs,
            verbose,
        ),
    }
}

fn main() -> std::io::Result<()> {
    run().map_err(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    })
}
