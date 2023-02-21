# amd-host-image-builder

This tool builds a flash image for an AMD Zen system.

# Setup

First, please set up the environment such that you have the required AMD firmware and also the bootloader you want to use.

This is done by executing the following command:

    git submodule update --init

# Usage

Then edit `etc/milan-gimlet-b.efs.json5` (or similar configuration file) to your liking.
Then, build an image for Milan by the following commands:

    make milan-gimlet-b.img

It's possible to specify `NANOBL_FLAGS_FOR_CARGO=...` at the end of that line in order to pass flags for the bootloader nanobl. An example would be to enable feature flags via `NANOBL_FLAGS_FOR_CARGO="-F <feature> ..."`.

If you do so, it's recommended to run `make -C nanobl-rs clean` beforehand since changes in those flags do not necessarily make nanobl rebuild things.

Or, if, instead of using the Makefile, you want to manually specify the command line to amd-host-image-builder, you can do this:

    target/amd-host-image-builder -c etc/milan-gimlet-b.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o milan-gimlet-b.img

Here, the configuration file used is `etc/milan-gimlet-b.efs.json5`, and the reset image is `nanobl-rs/obj/nanobl-rs.elf`. Only specially-prepared ELF images can be used here. `amd-host-image-builder` extracts the sections that need to be persistent from the ELF file and stores them into the appropriate entries of the flash. Those entries will automatically be created and should NOT be specified in the JSON configuration file.

You can also specify which directories are searched for blobs by passing `-B <directory>` (possibly multiple times) to `amd-host-image-builder`. For each blob name mentioned in the configuration, if that name is an absolute path then that will be used. Otherwise, the directories will be searched in the order they were specified.

The resulting image will be in `milan-gimlet-b.img` and can be flashed using [humility qspi](https://github.com/oxidecomputer/humility) or using a hardware flasher (CH341A etc).

The PSP will print debug messages to the serial port that can be configured in the settings below, see [PSP configuration](#psp-configuration).

# Configuration

The configuration file is JSON. `make` also builds a schema and stores it into file `efs.schema.json` in the project root directory. It is recommended to use an editor that can use JSON schemas to make editing the configuration easier (for example IntelliJ IDEA can be used--create a mapping between the file suffix `.efs.json5` and the file `efs.schema.json` in `JSON Schema Mappings` in its global settings).

The configuration contains: processor generation, psp directory and bhd directory.

Each directory has any number of entries.

Each entry has a `source` and a `target` field.

Use the `source` field to specify how to construct the payload data for that entry.
Possible fields in `source` are either `Value` (and an immediate value to use), `ApcbJson` (and the inline configuration for the PSP) or `BlobFile` (and the name of a file to load and use as payload).

Use the `target` field to specify where in the flash to put the result.
The only mandatory field here is `type` (to specify what kind of entry it is supposed to go to).

# PSP configuration

The PSP can be configured using one or multiple entries in the Bhd (yes, that's right) directory with the type `ApcbBackup` and/or `Apcb`. For now, it's recommended to have exactly one entry of type `ApcbBackup`, and to store it into `sub_program` 1 (that's an optional field in the `target`--which defaults to 0).

We represent the configuration of the PSP as JSON, too. The format is very close to the actual APCB (with some reserved fields being hidden if they are empty anyway). The checksum will automatically be updated by amd-host-image-builder and doesn't need to be manually updated.

For extra possible entries that you can add to the Apcb, please (make your editor) check the JSON schema.

The Apcb contains (about 3) groups. Each group contains several dozen entries. Each entry is either a Struct or a Tokens entry.

In general, Struct entries (entries which say `Struct` in the configuration) are older and AMD is in the process of replacing the settings with so-called Tokens entries (entries which say `tokens`).

For example, there's an entry `ErrorOutControl` which you can use to configure PSP error messages, and `ConsoleOutControl` which you can use to configure early PSP messages. You can configure target and verbosity.

Under `tokens`, there's are tokens to configure later PSP messages. For example, the token `AblSerialBaudRate` configures the baud rate of the UART, `FchConsoleOutMode` to set whether PSP prints to an UART or not (0), `FchConsoleOutSerialPort` to set which UART to use (`SuperIo`, `Uart0Mmio`, or `Uart1Mmio`)--although that supposedly moved to `FchConsoleMode` in Milan.

Settings should be set using the `tokens`, not the `Struct`s, if possible. The hope is that, one day, the structs will not be necessary at all anymore--and the token settings are preferred by the PSP anyhow.

# Preparation of ELF files

The PSP loads the reset image into memory like specified in the ELF file.

Afterwards, the PSP will start the x86 CPU in real mode, and that CPU will start executing 16 Byte from the *end* of the reset image.

That means that the ELF entry point needs to be 16 bytes from the end of the last segment--and that part needs to contain machine code that's valid for real mode.

Also, the ELF file needs to be linked in a way that it actually specified *physical* addresses. After all, there's no MMU set up yet--so virtual addresses don't make any sense (and we expect each virtual address to be equal to the same physical address for our purposes).

There should be ELF symbols `__sloader`, `__eloader` and `_BL_SPACE` available. Those are the expected start address of your program, the expected end address of your program, and the size of your loader program, respectively. The values of those special symbols are checked by `amd-host-image-builder` and it will fail if those are not what is expected.

As a special bringup help, right now, it's also possible to specify a non-ELF file. In that case, it will be put into x86 RAM such that it's right before address 0x8000_0000). Other checks are not done--you are on your own. We reserve the right to remove this weird non-ELF file support at any point.

# Other models

`amd-host-image-builder` also supports Rome. If you want to use that, please edit `etc/rome-ethanol-x.efs.json5` to your liking and then invoke `make rome-ethanol-x.img` to get `rome-ethanol-x.img` which you can flash.

# Using older configuration files

We had used JSON before, but now switched to JSON5.

In order to convert your old configuration files, you can use the following commands:

    sed -e 's;"\(0x[^"]*\)";\1;' -e 's;"\([a-zA-Z_][a-zA-Z0-9_]*\)":;\1:;' etc/Milan.json > etc/Milan.efs.json5
    rm etc/Milan.json

Please adapt the file names above as necessary.
