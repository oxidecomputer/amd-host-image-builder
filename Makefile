all: milan-ethanol-x rome-ethanol-x milan-gimlet-b
.PHONY: milan-ethanol-x rome-ethanol-x milan-gimlet-b all clean tests

CARGO = cargo

nanobl-rs/obj/nanobl-rs.elf:
	$(MAKE) -C nanobl-rs FLAGS_FOR_CARGO="$(NANOBL_FLAGS_FOR_CARGO)"

milan-ethanol-x: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- $(BLOB_DIRS:%=-B %) -B amd-firmware/GN/1.0.0.1 -B amd-firmware/GN/1.0.0.6 -c etc/milan-ethanol-x.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o milan-ethanol-x.img

rome-ethanol-x: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- $(BLOB_DIRS:%=-B %) -B amd-firmware/SSP/1.0.0.a -c etc/rome-ethanol-x.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o rome-ethanol-x.img

milan-gimlet-b: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- $(BLOB_DIRS:%=-B %) -B amd-firmware/GN/1.0.0.1 -B amd-firmware/GN/1.0.0.6 -c etc/milan-gimlet-b.efs.json5 -r nanobl-rs/obj/nanobl-rs.elf -o milan-gimlet-b.img

clean:
	rm -rf target
	$(MAKE) -C nanobl-rs clean

tests:
	$(CARGO) test
