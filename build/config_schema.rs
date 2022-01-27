use std::ffi::OsString;
use std::fs;
use std::path::Path;
use amd_host_image_builder_config::SerdeConfig;

pub fn generate_config_json_schema(outdir: &OsString) {
	let schema = schemars::schema_for!(SerdeConfig);
	let schema_file = Path::new(outdir).join("efs.schema.json");
	fs::write(schema_file, serde_json::to_string_pretty(&schema).unwrap()).unwrap();
}
