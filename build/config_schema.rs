use amd_host_image_builder_config::SerdeConfig;
use schemars::gen::SchemaSettings;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

pub fn generate_config_json_schema(outdir: &OsString) {
	let settings = SchemaSettings::default().with(|s| {
		// Work around schemars issue #62.
		// Downside: This makes the schema bigger by an order
		// of magnitude.
		s.inline_subschemas = true
	});
	let gen = settings.into_generator();
	let schema = gen.into_root_schema_for::<SerdeConfig>();

	let schema_file = Path::new(outdir).join("efs.schema.json");
	fs::write(schema_file, serde_json::to_string_pretty(&schema).unwrap())
		.unwrap();
}
