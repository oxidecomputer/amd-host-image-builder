mod config_schema;

use std::env;
use std::fs::{self};
use std::process;

fn main() {
	// OUT_DIR is set by Cargo and it's where any additional build artifacts
	// are written.
	let out_dir = match env::var_os("OUT_DIR") {
		Some(out_dir) => out_dir,
		None => {
			eprintln!("OUT_DIR environment variable not defined.");
			process::exit(1);
		}
	};
	fs::create_dir_all(&out_dir).unwrap();
	config_schema::generate_config_json_schema(&out_dir);

	// Make sure at least our default config in etc/Milan.json validates
	// using the schema we just generated.
	let schema_filename = format!("{}/{}", out_dir.into_string().unwrap(), "efs.schema.json");
	let schema_str = std::fs::read_to_string(schema_filename).unwrap();
	let schema_json: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
	//let schema_validator = JSONSchema::compile(&schema_json).unwrap();

	let configuration_filename = "etc/Milan.json";
	let configuration_str = std::fs::read_to_string(configuration_filename).unwrap();
	let configuration_json: serde_json::Value = serde_json::from_str(&configuration_str).unwrap();

	let schema_validator = jsonschema_valid::Config::from_schema(&schema_json, Some(jsonschema_valid::schemas::Draft::Draft6)).unwrap();

	if let Err(errors) = schema_validator.validate(&configuration_json) {
		for error in errors {
			eprintln!("validation error: {}", error);
		}
		std::process::exit(2);
	};
}
