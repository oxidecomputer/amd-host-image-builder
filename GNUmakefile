.PHONY: milan-ethanol-x milan-gimlet-b
.PHONY: all clean tests
.DELETE_ON_ERROR:

all: efs.schema.json milan-ethanol-x milan-gimlet-b

SOURCES:=	amd-host-image-builder-config/src/lib.rs \
		amd-host-image-builder-config/Cargo.toml \
		src/hole.rs \
		src/main.rs \
		src/static_config.rs \
		src/images.rs \
		Cargo.toml \
		Cargo.lock

PAYLOAD:=	PAYLOAD=/set/me

CARGO:= cargo

efs.schema.json: amd-host-image-builder-config/src/lib.rs \
  amd-host-image-builder-config/Cargo.toml \
  amd-host-image-builder-config/examples/amd-host-image-builder-schema.rs
	$(CARGO) run \
	    --manifest-path amd-host-image-builder-config/Cargo.toml \
	    --example amd-host-image-builder-schema > $@.new && \
	    mv $@.new $@

milan-ethanol-x-1.0.0.9.img: etc/milan-ethanol-x-1.0.0.9.efs.json5 \
  $(PAYLOAD) \
  amd-firmware/GN/1.0.0.9/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.9/PspBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.9/PspRecoveryBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.9/SmuFirmwareGn.csbin \
  amd-firmware/GN/1.0.0.9/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.9/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.9/SmuFirmware2Gn.csbin \
  amd-firmware/GN/1.0.0.9/SecureDebugUnlock_gn.sbin \
  amd-firmware/GN/1.0.0.9/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.9/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.9/RsmuSecPolicy_gn.sbin \
  amd-firmware/GN/1.0.0.9/Mp5Gn.csbin \
  amd-firmware/GN/1.0.0.9-fastspew/AgesaBootloader_U_prod_GN.csbin \
  amd-firmware/GN/1.0.0.9/GnPhyFw.sbin \
  amd-firmware/GN/1.0.0.9/PSP-Key-DB_gn.sbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_1D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_2D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.9/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.9-fastspew -B amd-firmware/GN/1.0.0.9 -c $< -r $(PAYLOAD) -o $@

milan-ethanol-x-1.0.0.a.img: etc/milan-ethanol-x-1.0.0.a.efs.json5 \
  $(PAYLOAD) \
  amd-firmware/GN/1.0.0.a/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.a/PspBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.a/PspRecoveryBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.a/SmuFirmwareGn.csbin \
  amd-firmware/GN/1.0.0.a/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.a/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.a/SmuFirmware2Gn.csbin \
  amd-firmware/GN/1.0.0.a/SecureDebugUnlock_gn.sbin \
  amd-firmware/GN/1.0.0.a/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.a/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.a/RsmuSecPolicy_gn.sbin \
  amd-firmware/GN/1.0.0.a/Mp5Gn.csbin \
  amd-firmware/GN/1.0.0.a/AgesaBootloader_U_prod_GN.csbin \
  amd-firmware/GN/1.0.0.a/GnPhyFw.sbin \
  amd-firmware/GN/1.0.0.a/PSP-Key-DB_gn.sbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.a -c $< -r $(PAYLOAD) -o $@

genoa-ruby-1.0.0.0.img: etc/genoa-ruby-1.0.0.0.efs.json5 \
  $(PAYLOAD) \
    amd-firmware/RS/1.0.0.0/AmdPubKey_rs.tkn \
    amd-firmware/RS/1.0.0.0/TypeId0x01_PspBl_RS.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x03_PspRecBl_RS.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x08_SmuFirmwareRS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x09_PspDebugUnlockToken_RS.stkn \
    amd-firmware/RS/1.0.0.0/TypeId0x0A_PspAblPubKey_RS.stkn \
    amd-firmware/RS/1.0.0.0/TypeId0x12_SmuFirmware2_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x13_SduFw_RS.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x21_PspAmdIkek_RS.bin \
    amd-firmware/RS/1.0.0.0/SecureEmptyToken.bin \
    amd-firmware/RS/1.0.0.0/TypeId0x24_RegisterAccessPolicy_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x2a_Mp5RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x30_AgesaBootLoaderU_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x42_PhyFw_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x50_PspKeyDataBase_RS.sbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x64_3_Rdimm_Imem1.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x65_3_Rdimm_Dmem1.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x64_4_Rdimm_Imem2.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x65_4_Rdimm_Dmem2.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x64_9_Rdimm_Imem1.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x65_9_Rdimm_Dmem1.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x64_A_Rdimm_Imem2.csbin \
    amd-firmware/RS/1.0.0.0/Appb_RS_Ddr5_0x65_A_Rdimm_Dmem2.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/RS/1.0.0.0 -c $< -r $(PAYLOAD) -o $@

milan-gimlet-b-1.0.0.a.img: etc/milan-gimlet-b-1.0.0.a.efs.json5 \
  $(PAYLOAD) \
  amd-firmware/GN/1.0.0.a/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.a/PspBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.a/PspRecoveryBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.a/SmuFirmwareGn.csbin \
  amd-firmware/GN/1.0.0.a/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.a/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.a/SmuFirmware2Gn.csbin \
  amd-firmware/GN/1.0.0.a/SecureDebugUnlock_gn.sbin \
  amd-firmware/GN/1.0.0.a/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.a/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.a/RsmuSecPolicy_gn.sbin \
  amd-firmware/GN/1.0.0.a/Mp5Gn.csbin \
  amd-firmware/GN/1.0.0.a/AgesaBootloader_U_prod_GN.csbin \
  amd-firmware/GN/1.0.0.a/GnPhyFw.sbin \
  amd-firmware/GN/1.0.0.a/PSP-Key-DB_gn.sbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.a/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.a -c $< -r $(PAYLOAD) -o $@


# For compatibility with previous versions of this tool
milan-gimlet-b: milan-gimlet-b-1.0.0.a.img
	@cp milan-gimlet-b-1.0.0.a.img milan-gimlet-b.img

# For compatibility with previous versions of this tool
milan-ethanol-x: milan-ethanol-x-1.0.0.a.img
	@cp milan-ethanol-x-1.0.0.a.img milan-ethanol-x.img

clean:
	$(RM) -rf target testpl testpl.o *.img

tests: testpl
	$(CARGO) test
	$(CARGO) run -- generate -B etc -c etc/test.json5 -o test.img -r testpl
	$(CARGO) run -- dump --existing-file=test.img | \
	    jq -r -e '.psp.PspDirectory.entries[].target.size' >/dev/null

ASFLAGS:=	--64

testpl: testpl.o testpl.ld
	$(LD) -o testpl -T testpl.ld testpl.o

dump-original:
	cargo run --  dump -i rrr/RRR1000F.FD -b rcb5
	# Automatically remove fixed flash_locations and Bios entries
	grep -v flash_location rcb5/config.efs.json5 | jq 'del(.. | objects | select(.target and .target.type == "Bios"))' > rcb5/new-config.efs.json5

remake: $(PAYLOAD)
	cargo run -- -c rcb5/new-config.efs.json5 -r $(PAYLOAD) -o /tmp/q5.img -B .
	cargo run --  dump -i /tmp/q5.img -b rcb5-inv

dump-original-turin:
	cargo run --  dump -i turin-rrr/RRRT0073D.FD -b tcb5
	# Automatically remove fixed flash_locations and Bios entries
	grep -v flash_location tcb5/config.efs.json5 | jq 'del(.. | objects | select(.target and .target.type == "Bios"))' > tcb5/new-config.efs.json5

remake-turin: $(PAYLOAD)
	cargo run -- -c tcb5/one.efs.json5 -o /tmp/q5.img -B .
	cargo run --  dump -i /tmp/q5.img -b tcb5-inv
