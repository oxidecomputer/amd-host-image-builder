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
    amd-firmware/RS/1.0.0.0/TypeId0x08_SmuFirmware_RSB0.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x08_SmuFirmware_Bergamo.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x09_PspDebugUnlockToken_RS.stkn \
    amd-firmware/RS/1.0.0.0/TypeId0x0A_PspAblPubKey_RS.stkn \
    amd-firmware/RS/1.0.0.0/TypeId0x12_SmuFirmware2_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x12_SmuFirmware2_RSB0.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x12_SmuFirmware2_Bergamo.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x13_SduFw_RS.sbin \
    amd-firmware/RS/1.0.0.0/TypeId0x21_PspAmdIkek_RS.bin \
    amd-firmware/RS/1.0.0.0/SecureEmptyToken.bin \
    amd-firmware/RS/1.0.0.0/TypeId0x24_RegisterAccessPolicy_RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x24_RegisterAccessPolicy_RSB0.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x24_RegisterAccessPolicy_Bergamo.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x2a_Mp5RS.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x2a_Mp5_RSB0.csbin \
    amd-firmware/RS/1.0.0.0/TypeId0x2a_Mp5_Bergamo.csbin \
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
	$(CARGO) run -- generate -s '16 MiB' $(BLOB_DIRS:%=-B %) -v -B amd-firmware/RS/1.0.0.0 -c $< -r $(PAYLOAD) -o $@

genoa-ruby-1.0.0.b.img: etc/genoa-ruby-1.0.0.b.efs.json5 \
  $(PAYLOAD) \
    amd-firmware/RS/1.0.0.b/AmdPubKey_rs.tkn \
    amd-firmware/RS/1.0.0.b/TypeId0x01_PspBl_RS.sbin \
    amd-firmware/RS/1.0.0.b/TypeId0x03_PspRecBl_RS.sbin \
    amd-firmware/RS/1.0.0.b/TypeId0x08_SmuFirmware_RSB0.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x08_SmuFirmware_Bergamo.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x09_PspDebugUnlockToken_RS.stkn \
    amd-firmware/RS/1.0.0.b/TypeId0x0A_PspAblPubKey_RS.stkn \
    amd-firmware/RS/1.0.0.b/TypeId0x12_SmuFirmware2_RSB0.sbin \
    amd-firmware/RS/1.0.0.b/TypeId0x12_SmuFirmware2_Bergamo.sbin \
    amd-firmware/RS/1.0.0.b/TypeId0x13_SduFw_RS.sbin \
    amd-firmware/RS/1.0.0.b/TypeId0x21_PspAmdIkek_RS.bin \
    amd-firmware/RS/1.0.0.b/SecureEmptyToken.bin \
    amd-firmware/RS/1.0.0.b/TypeId0x24_RegisterAccessPolicy_RSB0.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x24_RegisterAccessPolicy_Bergamo.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x2a_Mp5_RSB0.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x2a_Mp5_Bergamo.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x30_AgesaBootLoaderU_RS.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x42_PhyFw_RS.csbin \
    amd-firmware/RS/1.0.0.b/TypeId0x50_PspKeyDataBase_RS.sbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x64_3_Rdimm_Imem1.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x65_3_Rdimm_Dmem1.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x64_4_Rdimm_Imem2.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x65_4_Rdimm_Dmem2.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x64_9_Rdimm_Imem1.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x65_9_Rdimm_Dmem1.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x64_A_Rdimm_Imem2.csbin \
    amd-firmware/RS/1.0.0.b/Appb_RS_Ddr5_0x65_A_Rdimm_Dmem2.csbin \
  $(SOURCES)
	$(CARGO) run -- generate -s '16 MiB' $(BLOB_DIRS:%=-B %) -v -B amd-firmware/RS/1.0.0.b -c $< -r $(PAYLOAD) -o $@

turin-ruby-0.0.7.3.img: etc/turin-ruby-0.0.7.3.efs.json5 \
  $(PAYLOAD) \
    amd-firmware/BRH/0.0.7.3/TypeId0x00_AmdPubKey_BRH.tkn \
    amd-firmware/BRH/0.0.7.3/TypeId0x01_PspBl_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x08_SmuFirmware_breithorn.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x208_SmuFirmware_BRHDense.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x09_PspDebugUnlockToken_BRH.stkn \
    amd-firmware/BRH/0.0.7.3/TypeId0x0A_PspAblPubKey_BRH.stkn \
    amd-firmware/BRH/0.0.7.3/TypeId0x55_SPLTable_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x9D_AspSramFwExt_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x13_SduFw_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x22_SecureEmptyToken.bin \
    amd-firmware/BRH/0.0.7.3/TypeId0x24_RegisterAccessPolicy_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x224_RegisterAccessPolicy_BRHDense.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x28_PspSystemDriver_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x2A_SmuFirmware_breithorn.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x22A_SmuFirmware_BRHDense.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x2D_AblRt.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x30_AgesaBootLoaderU_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x42_PhyFw_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x44_USB_PHY_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x245_RegisterAccessPolicy_BRHDense.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x50_PspKeyDataBase_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x51_PspTosKeyDataBase_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x5DMpioFw_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x64_RasDriver_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x65_ta_ras_prod_amdTEE.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x73_PspBl_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x76_DfRib_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x8C_MPDMATF_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x91_GmiPhyFw_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x92_Page_BRH.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0x9F_psp_tos_wl_bin_brh.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0xA0_S3Image_BRH_A0.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0xA0_S3Image_BRHD_A0.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0xA0_S3Image_BRH_B0.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0xA0_S3Image_BRH_C0.sbin \
    amd-firmware/BRH/0.0.7.3/TypeId0xA0_S3Image_BRHD_B0.sbin \
    amd-firmware/BRH/0.0.7.3/APOB_NV_BRH.bin \
    amd-firmware/BRH/0.0.7.3/Type0x64_AppbDdr5RdimmImem3_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x64_AppbDdr5RdimmImem4_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x64_AppbDdr5RdimmPosttrainImem9_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x64_AppbDdr5RdimmPosttrainImem10_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x64_AppbDdr5RdimmQuickbootImem11_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmDmem3_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmDmem4_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmPosttrainDmem9_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmPosttrainDmem10_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmQuickbootDmem11_BRH.csbin \
    amd-firmware/BRH/0.0.7.3/Type0x65_AppbDdr5RdimmQuickbootDmem12_BRH.csbin \
  $(SOURCES)
	$(CARGO) run -- generate -s '16 MiB' $(BLOB_DIRS:%=-B %) -v -B . -B amd-firmware/BRH/0.0.7.3 -c $< -r $(PAYLOAD) -o $@

turin-ruby-0.0.8.1.img: etc/turin-ruby-0.0.8.1.efs.json5 \
  $(PAYLOAD) \
    amd-firmware/BRH/0.0.8.1/TypeId0x00_AmdPubKey_BRH.tkn \
    amd-firmware/BRH/0.0.8.1/TypeId0x01_PspBl_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x08_SmuFirmware_breithorn.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x208_SmuFirmware_BRHDense.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x09_PspDebugUnlockToken_BRH.stkn \
    amd-firmware/BRH/0.0.8.1/TypeId0x0A_PspAblPubKey_BRH.stkn \
    amd-firmware/BRH/0.0.8.1/TypeId0x55_SPLTable_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x9D_AspSramFwExt_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x13_SduFw_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x22_SecureEmptyToken.bin \
    amd-firmware/BRH/0.0.8.1/TypeId0x24_RegisterAccessPolicy_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x224_RegisterAccessPolicy_BRHDense.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x28_PspSystemDriver_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x2A_SmuFirmware_breithorn.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x22A_SmuFirmware_BRHDense.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x2D_AblRt.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x30_AgesaBootLoaderU_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x42_PhyFw_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x44_USB_PHY_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x245_RegisterAccessPolicy_BRHDense.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x50_PspKeyDataBase_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x51_PspTosKeyDataBase_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x5DMpioFw_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x64_RasDriver_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x65_ta_ras_prod_amdTEE.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x73_PspBl_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x76_DfRib_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x8C_MPDMATF_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x91_GmiPhyFw_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x92_Page_BRH.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0x9F_psp_tos_wl_bin_brh.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0xA0_S3Image_BRH_A0.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0xA0_S3Image_BRHD_A0.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0xA0_S3Image_BRH_B0.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0xA0_S3Image_BRH_C0.sbin \
    amd-firmware/BRH/0.0.8.1/TypeId0xA0_S3Image_BRHD_B0.sbin \
    amd-firmware/BRH/0.0.8.1/APOB_NV_BRH.bin \
    amd-firmware/BRH/0.0.8.1/Type0x64_AppbDdr5RdimmImem3_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x64_AppbDdr5RdimmImem4_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x64_AppbDdr5RdimmPosttrainImem9_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x64_AppbDdr5RdimmPosttrainImem10_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x64_AppbDdr5RdimmQuickbootImem11_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmDmem3_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmDmem4_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmPosttrainDmem9_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmPosttrainDmem10_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmQuickbootDmem11_BRH.csbin \
    amd-firmware/BRH/0.0.8.1/Type0x65_AppbDdr5RdimmQuickbootDmem12_BRH.csbin \
  $(SOURCES)
	$(CARGO) run -- generate -s '16 MiB' $(BLOB_DIRS:%=-B %) -v -B . -B amd-firmware/BRH/0.0.8.1 -c $< -r $(PAYLOAD) -o $@

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
