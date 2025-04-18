use amd_host_image_builder_config::SerdeConfig;
use schemars::r#gen::SchemaSettings;
use schemars::schema::RootSchema;
use std::path::Path;
use valico::json_schema;

pub fn generate_config_json_schema() -> RootSchema {
    let settings = SchemaSettings::default().with(|s| {
        // Work around schemars issue #62.
        // Downside: This makes the schema bigger by an order
        // of magnitude.
        s.inline_subschemas = true
    });
    let generator = settings.into_generator();
    generator.into_root_schema_for::<SerdeConfig>()
}

fn test_schema(schema_str: &str) {
    // Make sure our test efs config validates using the schema we just
    // generated.
    let schema_json: serde_json::Value =
        serde_json::from_str(schema_str).expect("Schema");
    let configuration_filename =
        Path::new("tests").join("data").join("Milan.efs.json5");
    let configuration_str =
        std::fs::read_to_string(configuration_filename).expect("configuration");
    let configuration_json: serde_json::Value =
        json5::from_str(&configuration_str)
            .expect("configuration be valid JSON");
    let mut scope = json_schema::Scope::new();
    let schema_validator = scope
        .compile_and_return(schema_json.clone(), false)
        .expect("schema be valid");
    let state = schema_validator.validate(&configuration_json);
    if !state.is_valid() {
        let errors = state.errors;
        for error in errors {
            eprintln!(
                "validation error: {}, {}, {:#?}",
                error,
                error.get_title(),
                error.get_detail()
            );
        }
        panic!("validation error");
    }
}

fn main() {
    let schema = generate_config_json_schema();
    let schema_string = serde_json::to_string_pretty(&schema).unwrap();
    test_schema(&schema_string);
    println!("{}", schema_string);
}
