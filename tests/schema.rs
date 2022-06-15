use std::path::Path;
use valico::json_schema;

#[test]
fn test_schema() {
	// Make sure at least our default config in etc/Milan.json validates
	// using the schema we just generated.
	let out_dir = match std::env::var_os("OUT_DIR") {
		Some(out_dir) => out_dir,
		None => {
			panic!("OUT_DIR environment variable not defined.");
		}
	};
	let schema_filename = Path::new(&out_dir).join("efs.schema.json");
	let schema_str = std::fs::read_to_string(schema_filename).unwrap();
	let schema_json: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
	let configuration_filename = Path::new("etc").join("Milan.efs.json5");
	let configuration_str = std::fs::read_to_string(configuration_filename).unwrap();
	let configuration_json: serde_json::Value = json5::from_str(&configuration_str).unwrap();
	let mut scope = json_schema::Scope::new();
	let schema_validator = scope.compile_and_return(schema_json.clone(), false).unwrap();
	let state = schema_validator.validate(&configuration_json);
	if !state.is_valid() {
		let errors = state.errors;
		for error in errors {
			eprintln!("validation error: {}, {}, {:#?}", error, error.get_title(), error.get_detail());
		}
		std::process::exit(2);
	}
}

