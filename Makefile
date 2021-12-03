
.PHONY: all
all:
	make -C nanobl-rs
	cargo run -- -g Milan -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img
	cargo run -- -g Rome -r nanobl-rs/obj/nanobl-rs.elf -o Rome.img
