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
# First, doctor the git submodule configuration:
#
sed -i -e 's,git@github.com:,https://github.com/,g' .gitmodules
git submodule sync
git submodule update --init

#
# Next, create a git(1) wrapper program.  This program will attempt to convert
# any SSH URLs found in the arguments to HTTPS URLs and then exec the real git.
#
mkdir -p /work/workaround
cat >/work/workaround/git <<'EOF'
#!/bin/bash
args=()
while (( $# > 0 )); do
	val="$1"
	val="${val//ssh:\/\/git@/https:\/\/}"
	val="${val//git@github.com:/https:\/\/github.com\/}"
	if [[ "$val" != "$1" ]]; then
		printf 'REGRET: transformed "%s" -> "%s"\n' "$1" "$val" >&2
	fi
	args+=( "$val" )
	shift
done
#
# Remove the workaround directory from PATH before executing the real git:
#
export PATH=${PATH/#\/work\/workaround:/}
exec /usr/bin/git "${args[@]}"
EOF
chmod +x /work/workaround/git
export PATH="/work/workaround:$PATH"

#
# Finally, require that cargo use the git CLI -- or, rather, our wrapper! --
# instead of the built-in support.  This achieves two things: first, SSH URLs
# should be transformed on fetch without requiring Cargo.toml rewriting, which
# is especially difficult in transitive dependencies; second, Cargo does not
# seem willing on its own to look in ~/.netrc and find the temporary token that
# buildomat generates for our job, so we must use git which uses curl.
#
export CARGO_NET_GIT_FETCH_WITH_CLI=true

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
