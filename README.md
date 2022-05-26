# amd-host-image-builder

This tool builds a flash image for an AMD Zen system.

# Usage

First, edit `etc/Milan.json` (or similar configuration file).
Then, build an image by the following commands:

    git submodule init
    git submodule update
    make

Or, if you want to manually specify the command line:

    target/amd-host-image-builder -c etc/Milan.json -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img

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

Settings should be set using the token, not the struct, if possible. The hope is that one day, the structs will not be necessary at all anymore--and the token settings are preferred by the PSP anyhow.
