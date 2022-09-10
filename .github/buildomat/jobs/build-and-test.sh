#!/bin/bash
#:
#: name = "build-and-test"
#: variety = "basic"
#: target = "helios"
#: rust_toolchain = "nightly-2021-09-01"
#: output_rules = [
#:	"/work/bins/*",
#: ]
#: access_repos = [
#:	"oxidecomputer/amd-apcb",
#:	"oxidecomputer/amd-efs",
#:	"oxidecomputer/amd-firmware",
#:	"oxidecomputer/amd-flash",
#:	"oxidecomputer/nanobl-rs",
#: ]
#:

set -o errexit
set -o pipefail
set -o xtrace

rustc --version
cargo --version

#
# The token authentication mechanism that affords us access to other private
# repositories requires that we use HTTPS URLs for GitHub, rather than SSH.
#
override_urls=(
    'git://github.com/'
    'git@github.com:'
    'ssh://github.com/'
    'ssh://git@github.com/'
)
for (( i = 0; i < ${#override_urls[@]}; i++ )); do
	git config --add --global url.https://github.com/.insteadOf \
	    "${override_urls[$i]}"
done

#
# Require that cargo use the git CLI instead of the built-in support.  This
# achieves two things: first, SSH URLs should be transformed on fetch without
# requiring Cargo.toml rewriting, which is especially difficult in transitive
# dependencies; second, Cargo does not seem willing on its own to look in
# ~/.netrc and find the temporary token that buildomat generates for our job,
# so we must use git which uses curl.
#
export CARGO_NET_GIT_FETCH_WITH_CLI=true

git submodule sync
git submodule update --init

banner test
ptime -m cargo test --verbose

banner build
ptime -m cargo build --release --verbose

banner package
mkdir -p /work/bins
for bin in amd-host-image-builder; do
	cp "target/release/$bin" "/work/bins/$bin"
	digest -a sha256 "/work/bins/$bin" > "/work/bins/$bin.sha256.txt"
	gzip "/work/bins/$bin"
	digest -a sha256 "/work/bins/$bin.gz" > "/work/bins/$bin.gz.sha256.txt"
done
