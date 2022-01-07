
.PHONY: all
all:
	make -C nanobl-rs
	cargo run -- -c etc/Milan.json -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img
	cargo run -- -c etc/Rome.json -r nanobl-rs/obj/nanobl-rs.elf -o Rome.img
