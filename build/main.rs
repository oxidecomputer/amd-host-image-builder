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
}
