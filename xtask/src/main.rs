// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//!
//! Build driver for AMD host image builder.
//!
use clap::Parser;
use duct::cmd;
use std::convert::From;
use std::env;
use std::path::{Path, PathBuf};

mod app;

#[derive(Parser)]
#[command(
    name = "ahib",
    author = "Oxide Computer Company",
    version = "0.1.0",
    about = "xtask build tool for AMD host image builder"
)]
struct Xtask {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Parser)]
enum Command {
    /// Builds AHIB.
    Build {
        #[clap(flatten)]
        profile: BuildProfile,
        #[clap(flatten)]
        features: Features,
        #[clap(flatten)]
        locked: Locked,
        #[clap(flatten)]
        verbose: Verbose,
    },
    /// Runs `cargo clean`
    Clean,
    /// Runs `cargo clippy` linter
    Clippy {
        #[clap(flatten)]
        features: Features,
        #[clap(flatten)]
        locked: Locked,
    },
    /// Runs AHIB for the specified target
    Gen {
        #[clap(long)]
        app: PathBuf,
        #[clap(long)]
        config: Option<PathBuf>,
        #[clap(long)]
        payload: PathBuf,
        #[clap(long)]
        amd_firmware: PathBuf,
        #[clap(long)]
        image: PathBuf,
        #[clap(flatten)]
        profile: BuildProfile,
        #[clap(flatten)]
        features: Features,
        #[clap(flatten)]
        locked: Locked,
        #[clap(flatten)]
        verbose: Verbose,
    },
    /// Dumps an existing image, extracting its blobs
    /// and printing its config
    Dump {
        #[clap(long)]
        image: String,
        #[clap(long)]
        blob_dir: String,
    },
    Schema,
    /// Runs unit tests
    Test {
        #[clap(flatten)]
        profile: BuildProfile,
        #[clap(flatten)]
        features: Features,
        #[clap(flatten)]
        locked: Locked,
        #[clap(flatten)]
        verbose: Verbose,
    },
}

/// BuildProfile defines whether we build in release or
/// debug mode.
#[derive(Default, Parser)]
struct BuildProfile {
    /// Build debug version (default)
    #[clap(long, conflicts_with_all = &["release"])]
    debug: bool,

    /// Build optimized version
    #[clap(long, conflicts_with_all = &["debug"])]
    release: bool,
}

#[derive(Parser, Default)]
struct Locked {
    /// Builds locked to Cargo.lock
    #[clap(long)]
    locked: bool,
}

#[derive(Parser, Default)]
struct Features {
    /// Compile-time features
    #[clap(long)]
    features: Option<String>,
}

#[derive(Parser, Default)]
struct Verbose {
    /// Compiles with the `--version` option
    #[clap(long)]
    verbose: bool,
}

/// Build arguments
#[derive(Debug, Default)]
struct Build {
    features: Option<String>,
    release: bool,
    locked: bool,
    verbose: bool,
}

impl Build {
    fn new(
        features: Features,
        profile: BuildProfile,
        locked: Locked,
        verbose: Verbose,
    ) -> Build {
        Build {
            features: features.features,
            release: profile.release,
            locked: locked.locked,
            verbose: verbose.verbose,
        }
    }
    fn cmd_str(&self, verb: &str) -> String {
        format!(
            "{verb} {locked} {verbose} {profile} {features}",
            locked = if self.locked { "--locked" } else { "" },
            verbose = if self.verbose { "--verbose" } else { "" },
            profile = if self.release { "--release" } else { "" },
            features = self
                .features
                .as_ref()
                .map(|features| format!("--features={features}"))
                .unwrap_or("".into()),
        )
    }

    fn _dir(&self) -> &'static Path {
        Path::new(if self.release { "release" } else { "debug" })
    }
}

fn main() -> std::io::Result<()> {
    let xtask = Xtask::parse();
    match xtask.cmd {
        Command::Build { profile, features, locked, verbose } => {
            build(Build::new(features, profile, locked, verbose));
        }
        Command::Clippy { features, locked } => clippy(Build::new(
            features,
            BuildProfile::default(),
            locked,
            Verbose::default(),
        )),
        Command::Clean => clean(),
        Command::Dump { image, blob_dir } => {
            dump(&image, &blob_dir);
        }
        Command::Gen {
            app,
            config,
            payload,
            amd_firmware,
            image,
            features,
            profile,
            locked,
            verbose,
        } => run_gen(
            app.as_path(),
            config.as_deref(),
            payload.as_path(),
            amd_firmware.as_path(),
            image.as_path(),
            Build::new(features, profile, locked, verbose),
        ),
        Command::Schema => schema(),
        Command::Test { profile, features, locked, verbose } => {
            tests(Build::new(features, profile, locked, verbose))
        }
    }
    Ok(())
}

/// Runs a cross-compiled build.
fn build(args: Build) {
    cmd(cargo(), args.cmd_str("build").split_whitespace())
        .run()
        .expect("build successful");
}

fn run_gen<P: AsRef<Path> + ?Sized>(
    app: &P,
    config: Option<&P>,
    payload: &P,
    amd_firmware: &P,
    image: &P,
    args: Build,
) {
    let app = app::try_from_file(app.as_ref()).expect("Parsed app");
    let name = app.name();
    let name = Path::new(&name);
    let run = args.cmd_str("run");
    let blob_dir = app.blob_path(amd_firmware.as_ref());
    let mut config = if let Some(c) = config {
        PathBuf::from(c.as_ref())
    } else {
        let mut c = PathBuf::from("etc").join(name);
        c.set_extension("efs.json5");
        c
    };

    if let Some(patch) = app.patch() {
        let mut patch_config = PathBuf::from("target");
        patch_config.push(config.strip_prefix("etc").unwrap());
        config = patch_config;
        cmd!(
            "patch",
            "-F",
            "0",
            "-l",
            "-N",
            "-o",
            config.to_str().unwrap(),
            patch.base().to_str().unwrap(),
            patch.diff().to_str().unwrap()
        )
        .run()
        .expect("patch applied");
    }

    let mut missing =
        app.blobs().iter().filter(|&f| !blob_dir.join(f).exists()).peekable();
    if missing.peek().is_some() {
        for file in missing {
            eprintln!("blob file {file:?} not found or readable");
        }
        std::process::exit(1);
    }

    let size = app.size();
    let payload = payload.as_ref().to_string_lossy();
    let config = config.to_string_lossy();
    let image = image.as_ref().to_string_lossy();
    let blob_dir = blob_dir.to_string_lossy();

    let mut args = run.split_whitespace().collect::<Vec<_>>();
    args.extend(["--", "generate", "-v"]);
    args.extend(["-s", &size]);
    args.extend(["-r", &payload]);
    args.extend(["-c", &config]);
    args.extend(["-B", &blob_dir]);
    args.extend(["-o", &image]);
    cmd(cargo(), args).run().expect("run successful");
}

fn dump<P: AsRef<Path> + ?Sized>(image: &P, blob_dir: &P) {
    let args = format!(
        "run -- dump -i {image} -b {blob_dir}",
        image = image.as_ref().display(),
        blob_dir = blob_dir.as_ref().display()
    );
    cmd(cargo(), args.split_whitespace()).run().expect("dump successful");
}

/// Generates a JSON schema for our config.
fn schema() {
    let out = std::fs::File::create("out/efs.schema.json")
        .expect("created schema file");
    cmd(cargo(), ["run", "--manifest-path", "ahib-schema/Cargo.toml", "--"])
        .stdout_file(out)
        .run()
        .expect("generated schema");
}

/// Runs unit tests.
fn tests(args: Build) {
    cmd(cargo(), args.cmd_str("test --workspace").split_whitespace())
        .run()
        .expect("test successful");
    test_payload();
}

/// Runs the Clippy linter.
fn clippy(args: Build) {
    cmd(cargo(), args.cmd_str("clippy --workspace").split_whitespace())
        .run()
        .expect("clippy successful");
}

/// Runs clean on the project.
fn clean() {
    cmd!(cargo(), "clean").run().expect("clean successful");
}

/// Returns the value of the given environment variable,
/// or the default if unspecified.
fn env_or(var: &str, default: &str) -> String {
    env::var(var).unwrap_or(default.into())
}

/// Returns the name of the cargo binary.
fn cargo() -> String {
    env_or("CARGO", "cargo")
}

fn test_payload() {
    let image = "target/test.img";
    run_gen(
        "apps/test.toml",
        None,
        "target/testpl",
        "tests",
        image,
        Build::default(),
    );
    dump(image, "target/test_blobs");
}
