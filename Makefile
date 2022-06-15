all: milan rome
.PHONY: rome milan all clean tests

CARGO = cargo

nanobl-rs/obj/nanobl-rs.elf:
	make -C nanobl-rs

milan: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- -c etc/Milan.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o Milan.img

rome: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- -c etc/Rome.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o Rome.img

clean:
	rm -rf target

tests:
	$(CARGO) test
