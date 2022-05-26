# amd-host-image-builder

This tool builds a flash image for an AMD Zen system.

# Setup

First, please set up the environment such that you have the reqired AMD firmware and also the bootloader you want to use.

This is done by executing the following commands:

    git submodule init
    git submodule update

# Usage

Then edit `etc/Milan.json` (or similar configuration file) to your liking.
Then, build an image for Milan.img by the following commands:

    make milan

Or, if you want to manually specify the command line:

    target/amd-host-image-builder -c etc/Milan.json -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img

Here, the configuration file used is `etc/Milan.json`, and the reset image is `nanobl-rs/obj/nanobl-rs.elf`. Only specially-prepared ELF images can be used here. `amd-host-image-builder` extracts the sections that need to be persistent from the ELF file and stores them into the appropriate entries of the flash. Those entries will automatically be created and should NOT be specified in the JSON configuration file.

The resulting image will be in `Milan.img` and can be flashed using [Humility](https://github.com/oxidecomputer/humility) or using a hardware flasher (CH341A etc).

The PSP will print debug messages to the serial port that can be configured in the settings below, see [PSP configuration].

# Configuration

The configuration file is JSON. `make` also builds a schema and stores it into file `efs.schema.json`. It is recommended to use an editor that can use JSON schemas to help the author.

The configuration contains: processor generation, psp directory and bhd directory.

Each directory has any number of entries.

Each entry has a `source` and a `target` field.

Use the `source` field to specify how to construct the payload data for that entry.
Possible fields in `source` are either `Value`, `ApcbJson` (and the inline configuration for the PSP) or `BlobFile` (and the name of a file to load and use as payload).

Use the `target` field to specify where in the flash to put the result.
Mandatory fields there are `type` (to specify what kind of entry it is supposed to go to), and either `Blob` or `Value`.
Most of the field types need a `Blob`, except for `PspSoftFuseChain`, which needs a `Value`.

# PSP configuration

The PSP can be configured using one or multiple entries in the Bhd (yes, that's right) directory with the type `ApcbBackup` and/or `Apcb`. For now, it's recommended to have exactly one entry of type `ApcbBackup`, and to store it into `sub_program` 1 (that's an optional field in the `target`--which defaults to 0).

We represent the configuration of the PSP as JSON, too. The format is very close to the actual APCB (with some reserved fields being hidden if they are empty anyway). The checksum will automatically be updated by amd-host-image-builder and doesn't need to be manually updated.

For extra possible entries that you can add to the Apcb, please (make your editor) check the JSON schema.

The Apcb contains (about 3) groups. Each group contains several dozen entries. Each entry is either a Struct or a Tokens entry.

In general, Struct entries (entries which say `Struct` in the configuration) are older and AMD is in the process of replacing the settings with so-called Tokens entries (entries which say `tokens`).

For example, there's an entry `ErrorOutControl` which you can use to configure PSP error messages, and `ConsoleOutControl` which you can use to configure early PSP messages. You can configure target and verbosity.

Under `tokens`, there's are tokens to configure later PSP messages. For example, the token `AblSerialBaudRate` configures the baud rate of the UART, `FchConsoleOutMode` to set whether PSP prints to an UART or not (0), `FchConsoleOutSerialPort` to set which UART to use (`SuperIo`, `Uart0Mmio`, or `Uart1Mmio`)--although that supposedly moved to `FchConsoleMode` in Milan.

Settings should be set using the token, not the struct, if possible. The hope is that, one day, the structs will not be necessary at all anymore--and the token settings are preferred by the PSP anyhow.

# Preparation of ELF files

The PSP loads the reset image into memory like specified in the ELF file.

Afterwards, the PSP will start the x86 CPU in real mode, and that CPU will start executing 16 Byte from the *end* of the reset image.

That means that the ELF entry point needs to be 16 bytes from the end of the last segment--and that part needs to contain machine code that's valid for real mode.

Also, the ELF file needs to be linked in a way that it actually specified *physical* addresses. After all, there's no MMU set up yet--so virtual addresses don't make any sense (and we expect each virtual address to be equal to the same physical address for our purposes).

There should be ELF symbols `__sloader`, `__eloader` and `_BL_SPACE` available. Those are the expected start address of your program, the expected end address of your program, and the size of your loader program, respectively. The values of those special symbols are checked by `amd-host-image-builder` and it will fail if those are not what is expected.

As a special bringup help, right now, it's also possible to specify a non-ELF file. In that case, it will be put into x86 RAM such that it's right before address 0x8000_0000). Other checks are not done--you are on your own. We reserve the right to remove this weird non-ELF file support at any point.

# Other models

`amd-host-image-builder` also supports Rome. If you want to use that, please edit `etc/Rome.json` to your liking and then invoke `make rome` to get `Rome.img` which you can flash.
