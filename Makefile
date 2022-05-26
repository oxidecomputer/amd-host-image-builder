all: milan rome
.PHONY: rome milan all clean

nanobl-rs/obj/nanobl-rs.elf:
	make -C nanobl-rs

milan: nanobl-rs/obj/nanobl-rs.elf
	cargo run -- -c etc/Milan.json -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img

rome: nanobl-rs/obj/nanobl-rs.elf
	cargo run -- -c etc/Rome.json -r nanobl-rs/obj/nanobl-rs.elf -o Rome.img

clean:
	rm -rf target
