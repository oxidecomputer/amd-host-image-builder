#!/bin/bash
#:
#: name = "build-and-test"
#: variety = "basic"
#: target = "helios"
#: rust_toolchain = "stable"
#: output_rules = [
#:	"/work/bins/*",
#: ]
#: access_repos = [
#:	"oxidecomputer/amd-apcb",
#:	"oxidecomputer/amd-efs",
#: ]
#:

set -o errexit
set -o pipefail
set -o xtrace

export LD=gld

gld --version
rustc --version
cargo --version

banner build
ptime -m cargo xtask build --release --verbose --locked
banner package

mkdir -p /work/bins
bin=amd-host-image-builder
cp "target/release/$bin" "/work/bins/$bin"
digest -a sha256 "/work/bins/$bin" > "/work/bins/$bin.sha256.txt"
gzip "/work/bins/$bin"
digest -a sha256 "/work/bins/$bin.gz" > "/work/bins/$bin.gz.sha256.txt"
