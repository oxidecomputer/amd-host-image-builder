
.PHONY: all
all:
	cargo run -- -g Milan -r nanobl-rs-0x7ffc_d000.bin -o Milan.img
	cargo run -- -g Rome -r nanobl-rs-0x7ffc_d000.bin -o Rome.img
