all: efs.schema.json milan-ethanol-x rome-ethanol-x milan-gimlet-b
.PHONY: milan-ethanol-x milan-ethanol-x-1.0.0.9 rome-ethanol-x milan-gimlet-b
.PHONY: all clean tests
.PHONY: FRC
.DELETE_ON_ERROR:

SOURCES = amd-host-image-builder-config/src/lib.rs \
    src/hole.rs src/main.rs src/static_config.rs src/images.rs \
    Cargo.toml amd-host-image-builder-config/Cargo.toml Cargo.lock

CARGO = cargo

FRC:

nanobl-rs/obj/nanobl-rs.elf: FRC
	$(MAKE) -C nanobl-rs FLAGS_FOR_CARGO="$(NANOBL_FLAGS_FOR_CARGO)"

efs.schema.json: amd-host-image-builder-config/src/lib.rs amd-host-image-builder-config/Cargo.toml amd-host-image-builder-config/examples/amd-host-image-builder-schema.rs
	$(CARGO) run --manifest-path amd-host-image-builder-config/Cargo.toml --example amd-host-image-builder-schema > $@.new && mv $@.new $@

milan-ethanol-x.img: etc/milan-ethanol-x.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
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
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.9-fastspew -B amd-firmware/GN/1.0.0.9 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-ethanol-x-1.0.0.1.img: etc/milan-ethanol-x-1.0.0.1.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
  amd-firmware/GN/1.0.0.1/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.1/PspBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.1/PspRecoveryBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.1/SmuFirmwareGn.esbin \
  amd-firmware/GN/1.0.0.1/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.1/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.1/SmuFirmware2Gn.esbin \
  amd-firmware/GN/1.0.0.1/SecureDebugUnlock_gn.esbin \
  amd-firmware/GN/1.0.0.1/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.1/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.1/RsmuSecPolicy_gn.esbin \
  amd-firmware/GN/1.0.0.1/Mp5Gn.esbin \
  amd-firmware/GN/1.0.0.1/AgesaBootloader_U_prod_GN.cesbin \
  amd-firmware/GN/1.0.0.1/GnPhyFw.cesbin \
  amd-firmware/GN/1.0.0.1/GnKeyDb.stkn \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Rdimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Rdimm_Dmem.ecsbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.1 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-ethanol-x-1.0.0.2.img: etc/milan-ethanol-x-1.0.0.2.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
  amd-firmware/GN/1.0.0.2/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.2/PspBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.2/PspRecoveryBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.2/SmuFirmwareGn.esbin \
  amd-firmware/GN/1.0.0.2/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.2/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.2/SmuFirmware2Gn.esbin \
  amd-firmware/GN/1.0.0.2/SecureDebugUnlock_gn.esbin \
  amd-firmware/GN/1.0.0.2/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.2/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.2/RsmuSecPolicy_gn.esbin \
  amd-firmware/GN/1.0.0.2/Mp5Gn.esbin \
  amd-firmware/GN/1.0.0.2/AgesaBootloader_U_prod_GN.cesbin \
  amd-firmware/GN/1.0.0.2/GnPhyFw.cesbin \
  amd-firmware/GN/1.0.0.2/PSP-Key-DB_gn.sbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_1D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_1D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_1D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_1D_Ddr4_Rdimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_2D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_2D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_2D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.2/Appb_GN_2D_Ddr4_Rdimm_Dmem.ecsbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.2 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-ethanol-x-1.0.0.4.img: etc/milan-ethanol-x-1.0.0.4.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
  amd-firmware/GN/1.0.0.4/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.4/PspBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.4/PspRecoveryBootLoader_gn.sbin \
  amd-firmware/GN/1.0.0.4/SmuFirmwareGn.csbin \
  amd-firmware/GN/1.0.0.4/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.4/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.4/SmuFirmware2Gn.csbin \
  amd-firmware/GN/1.0.0.4/SecureDebugUnlock_gn.sbin \
  amd-firmware/GN/1.0.0.4/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.4/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.4/RsmuSecPolicy_gn.sbin \
  amd-firmware/GN/1.0.0.4/Mp5Gn.csbin \
  amd-firmware/GN/1.0.0.4/AgesaBootloader_U_prod_GN.csbin \
  amd-firmware/GN/1.0.0.4/GnPhyFw.sbin \
  amd-firmware/GN/1.0.0.4/PSP-Key-DB_gn.sbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_1D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_1D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_1D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_1D_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_2D_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_2D_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_2D_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.4/Appb_GN_2D_Ddr4_Rdimm_Dmem.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.4 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-ethanol-x-1.0.0.9.img: etc/milan-ethanol-x-1.0.0.9.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
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
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.9-fastspew -B amd-firmware/GN/1.0.0.9 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-ethanol-x-1.0.0.a.img: etc/milan-ethanol-x-1.0.0.a.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
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
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.a -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

rome-ethanol-x.img: etc/rome-ethanol-x.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
  amd-firmware/SSP/1.0.0.a/AmdPubKey_ssp.bin \
  amd-firmware/SSP/1.0.0.a/PspBootLoader_ssp.sbin \
  amd-firmware/SSP/1.0.0.a/PspRecoveryBootLoader_ssp.sbin \
  amd-firmware/SSP/1.0.0.a/SmuFirmwareSsp.csbin \
  amd-firmware/SSP/1.0.0.a/AblPubKey_ssp.bin \
  amd-firmware/SSP/1.0.0.a/SmuFirmware2Ssp.csbin \
  amd-firmware/SSP/1.0.0.a/SecureDebugUnlock_ssp.sbin \
  amd-firmware/SSP/1.0.0.a/PspIkek_ssp.bin \
  amd-firmware/SSP/1.0.0.a/SecureEmptyToken.bin \
  amd-firmware/SSP/1.0.0.a/RsmuSecPolicy_ssp.sbin \
  amd-firmware/SSP/1.0.0.a/Mp5Ssp.csbin \
  amd-firmware/SSP/1.0.0.a/AgesaBootloader_U_prod_Mcm_SSP.bin \
  amd-firmware/SSP/1.0.0.a/SspPhyFw.sbin \
  amd-firmware/SSP/1.0.0.a/SspPhyFwSb4kr.stkn \
  amd-firmware/SSP/1.0.0.a/Starship-PMU-FW.stkn \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_1D_ddr4_Udimm_Imem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_1D_ddr4_Udimm_Dmem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_1D_Ddr4_Rdimm_Imem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_1D_Ddr4_Rdimm_Dmem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_2D_Ddr4_Udimm_Imem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_2D_Ddr4_Udimm_Dmem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_2D_Ddr4_Rdimm_Imem.cbin \
  amd-firmware/SSP/1.0.0.a/Appb_SSP_2D_Ddr4_Rdimm_Dmem.cbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/SSP/1.0.0.a -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-gimlet-b-1.0.0.1.img: etc/milan-gimlet-b-1.0.0.1.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
  amd-firmware/GN/1.0.0.1/AmdPubKey_gn.tkn \
  amd-firmware/GN/1.0.0.1/PspBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.1/PspRecoveryBootLoader_gn.esbin \
  amd-firmware/GN/1.0.0.1/SmuFirmwareGn.esbin \
  amd-firmware/GN/1.0.0.1/SecureDebugToken_gn.stkn \
  amd-firmware/GN/1.0.0.1/PspABLFw_gn.stkn \
  amd-firmware/GN/1.0.0.1/SmuFirmware2Gn.esbin \
  amd-firmware/GN/1.0.0.1/SecureDebugUnlock_gn.esbin \
  amd-firmware/GN/1.0.0.1/PspIkek_gn.bin \
  amd-firmware/GN/1.0.0.1/SecureEmptyToken.bin \
  amd-firmware/GN/1.0.0.1/RsmuSecPolicy_gn.esbin \
  amd-firmware/GN/1.0.0.1/Mp5Gn.esbin \
  amd-firmware/GN/1.0.0.1/AgesaBootloader_U_prod_GN.cesbin \
  amd-firmware/GN/1.0.0.1/GnPhyFw.cesbin \
  amd-firmware/GN/1.0.0.1/GnKeyDb.stkn \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_1D_Ddr4_Rdimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Udimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Udimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Rdimm_Imem.ecsbin \
  amd-firmware/GN/1.0.0.1/Appb_GN_2D_Ddr4_Rdimm_Dmem.ecsbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Udimm_Imem.csbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Udimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Rdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Rdimm_Dmem.csbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Lrdimm_Imem.csbin \
  amd-firmware/GN/1.0.0.6/Appb_GN_BIST_Ddr4_Lrdimm_Dmem.csbin \
  $(SOURCES)
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.1 -B amd-firmware/GN/1.0.0.6 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-gimlet-b-1.0.0.9.img: etc/milan-gimlet-b.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
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
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.9-fastspew -B amd-firmware/GN/1.0.0.9 -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@

milan-gimlet-b-1.0.0.a.img: etc/milan-gimlet-b.efs.json5 nanobl-rs/obj/nanobl-rs.elf \
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
	$(CARGO) run -- generate $(BLOB_DIRS:%=-B %) -v -B amd-firmware/GN/1.0.0.a -c $< -r nanobl-rs/obj/nanobl-rs.elf -o $@


# For compatibility with previous versions of this tool
milan-ethanol-x: milan-ethanol-x.img

# For compatibility with previous versions of this tool
rome-ethanol-x: rome-ethanol-x.img

# For compatibility with previous versions of this tool
milan-gimlet-b: milan-gimlet-b-1.0.0.a.img
	@cp milan-gimlet-b-1.0.0.a.img milan-gimlet-b.img

# For compatibility with previous versions of this tool
milan-ethanol-x-1.0.0.9: milan-ethanol-x-1.0.0.9.img

clean:
	rm -rf target
	$(MAKE) -C nanobl-rs clean

tests: nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) test
	$(CARGO) run -- generate -B etc -c etc/test.json5 -o test.img -r nanobl-rs/obj/nanobl-rs.elf
	$(CARGO) run -- dump --existing-file=test.img | jq -r -e '.psp.PspDirectory.entries[].target.size' >/dev/null
