# amd-host-image-builder

This tool builds flash images for AMD Zen systems.

# Setup

You must use GNU make to _run_ `amd-host-image-builder` in the
provided example configurations.

Make sure that you have GNU `ld` and `as` available for building
the test payload if you intend to run tests.

Run `git submodule update --init` to download AMD firmware blobs
used when running the tool.

# Usage

Select a configuration file for the system and firmware version
that you are using, e.g., `etc/milan-gimlet-b-1.0.0.a.efs.json5`
and customize it according to your application if necessary.

To build an image for a Milan-based Gimlet, run the following,
specifying the `make` variable `PAYLOAD` to point to the reset
image payload you want in the result:

    gmake milan-gimlet-b PAYLOAD=/path/to/phbl

or,

    gmake milan-gimlet-b PAYLOAD=/path/to/bldb/target/x86_64-oxide-none-elf/release/bldb

This will create two image files in the current directory, one
with the current firmware version, and one without:

    milan-gimlet-b-1.0.0.a.img
    milan-gimlet-b.img

One may also run the command via `cargo run`.  For example:

    cargo run -- \
        -B amd-firmware/GN/1.0.0.a \
        -c etc/milan-gimlet-b-1.0.0.a.efs.json5 \
        -r /path/to/target/x86_64-oxide-none-elf/release/bldb \
        -o milan-gimlet-b-1.0.0.a.img

Here, the following are given as arguments:

* `amd-firmware/GN/1.0.0.a` is in the search path for blobs,
* The configuration file used is `etc/milan-gimlet-b-1.0.0.a.efs.json5`,
* The reset image is `bldb`.  Note that there are restrictions on the
  contents of the ELF file given to the image builder.
  `amd-host-image-builder` extracts the segments that need to
  be in the reset image from the ELF file and stores them into the
  appropriate flash entries.  Those entries will automatically be created
  and should _not_ be specified in the JSON configuration file.
* The output file name is given as `milan-gimlet-b-1.0.0.a.img`.

`amd-host-image-builder` will incorporate a number of
(necessary) blobs named in the configuration file.  These blobs
are located by searching directories specified via the `-B
<directory>` option, which can be given multiple times.  If a
blob is named by an absolute path in the configuration file,
then that will be used. Otherwise, the directories will be
searched in the order they were specified.

The resulting image will be in `milan-gimlet-b-1.0.0.a.img` and
can be flashed using
[humility qspi](https://github.com/oxidecomputer/humility) or
using a hardware flasher (CH341A etc).

The PSP will print debug messages to the serial port that can be
configured in the settings below, see [PSP configuration](#psp-configuration).

# Configuration

The configuration file syntax is JSON5.

The configuration file contents specify:
* Processor generation,
* PSP directory contents,
* BHD directory contents,

See the subsections below for more details.

The Makefile includes a target that builds a schema and stores
it into file `efs.schema.json` in the project root directory.
We recommend using an editor that can match the document against
a schema when editing the configuration.  For example, IntelliJ
IDEA is known to work by setting a mapping between the file
suffix `.efs.json5` and the file `efs.schema.json` in `JSON
Schema Mappings` in its global settings.

## Directory Configuration

Each directory has any number of entries.  Each entry has a
`source` and a `target` field.

The `source` field specifies how to construct the payload
data for that entry.  Possible fields in `source` are:

* `Value` and an immediate value to use
* `ApcbJson` and the inline configuration for the PSP
* `BlobFile` and the name of a file to load and use as payload

Use the `target` field to specify where in the flash to put the
result.  The only mandatory field is `type` to specify the
corresponding entry kind.

## PSP configuration

The PSP can be configured using one or multiple entries in the
Bhd directory with the type `ApcbBackup` and/or `Apcb`.  For
now, it is recommended to have exactly one entry of type
`ApcbBackup`, and to store it into `sub_program` 1.  Note that
`sub_program` an optional field in the `target` that defaults to
0.

We represent the PSP configuration as JSON5, too. The format is
very close to the actual APCB, with some reserved fields hidden
if they are empty anyway.  The checksum will automatically be
updated by amd-host-image-builder and doesn't need to be
manually updated.

For extra possible entries that you can add to the Apcb, be sure
to validate against the JSON schema.

The Apcb contains (about 3) groups. Each group contains several
dozen entries.  Each entry is either a "struct" or a "tokens"
entry.  Struct entries are denoted by the keyword `Struct`, and
are an older mechanism.  AMD is in the process of replacing them
with tokens entries, denoted by `tokens`.  Settings should be
made using `tokens` entries, not `Struct` entries, if possible.

For example, there's an entry `ErrorOutControl` which you can
use to configure PSP error messages, and `ConsoleOutControl`
which you can use to configure early PSP messages. You can
configure target and verbosity.

Under `tokens`, there's are tokens to configure later PSP
messages.  For example, the token `AblSerialBaudRate` configures
the baud rate of the UART, `FchConsoleOutMode` to set whether
PSP prints to a UART or not (0), `FchConsoleOutSerialPort` to
set which UART to use (`SuperIo`, `Uart0Mmio`, or `Uart1Mmio`;
supposedly moved to `FchConsoleMode` in Milan.

# Preparation of ELF files

This tool generates its output such that, when the PSP loads the
reset image into memory, it is laid out as specified in the
source ELF file.  Execution will start at the ELF entry point.

When the system starts, the PSP holds the x86 cores in reset
while loading the image.  Eventually, it starts the x86 "boot"
CPU (called the "Boot Support Core", "Bootstrap Core", or BSC in
AMD's documentation) in 16-bit real mode.  That CPU will start
executing 16 Byte below the end of the aligned 16-bit real-mode
segment at the end of the reset image.  Thus, the ELF entry
point must contain valid 16-bit real-mode code exactly 16 bytes
from the end of the last segment in the file.

Also, the ELF file needs to be linked in a way that it actually
specifies _physical_ addresses: there is no MMU configured, let
alone enabled, when execution starts at the reset vector.

The ELF file must also expose three symbols:
* `__sloader` marking the start of the reset image
* `__eloader` marking the end of the reset image
* `_BL_SPACE` giving the amount of memory required for
  the image, in bytes.

The values of those special symbols are validated by
`amd-host-image-builder`, which will fail if they do not
match a set of hardcoded criteria.  See the tool's source
code for the exact rules.

For the special case of facilitating bring-up work, it is also
possible to specify a non-ELF file that is interpreted basically
as a blob. In that case, it will be loaded into RAM such that it
ends at address 0x8000_0000.  Validation checks are not
performed in this case.  Further, we make no claims of stability
for this feature and reserve the right to change or remove it at
any point--you are on your own.

# Using older configuration files

We previously used JSON for the configuration file syntax, but
switched to JSON5, which supports non-strings as keys, trailing
commas, comments, and hexadecimal notation for integer
constants.  If you find an older file that you could like to
convert to the newer format, you can use the following `sed`
script invocation:

    sed \
        -e 's;"\(0x[^"]*\)";\1;' \
        -e 's;"\([a-zA-Z_][a-zA-Z0-9_]*\)":;\1:;' \
        etc/Milan.json > etc/Milan.efs.json5

Please note the file name suffix change!
