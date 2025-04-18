// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use duct::cmd;
use std::env;

fn link(args: &[&str]) {
    let ld: String = env::var("LD").unwrap_or("ld.lld".into());
    cmd(ld, args)
        .run()
        .or_else(|_| cmd("gld", args).run())
        .or_else(|_| cmd("ld.lld", args).run())
        .or_else(|_| cmd("ld", args).run())
        .expect("linked testpl");
}

fn main() {
    println!("cargo:rerun-if-changed=testpl.ld");
    let out = env::var("CARGO_TARGET_DIR").unwrap_or("../target".into());
    let testpl = format!("{out}/testpl");
    let mut args = vec!["-T", "testpl.ld", "-o", &testpl];
    let objs = cc::Build::new()
        .target("x86_64-elf-none")
        .file("testpl.s")
        .compile_intermediates();
    args.extend(objs.iter().map(|p| p.to_str().unwrap()));
    link(&args);
}
