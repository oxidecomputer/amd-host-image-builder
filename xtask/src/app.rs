// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Eq, PartialEq)]
enum Cpu {
    #[serde(rename = "rome")]
    Rome,
    #[serde(rename = "milan")]
    Milan,
    #[serde(rename = "genoa")]
    Genoa,
    #[serde(rename = "turin")]
    Turin,
    #[serde(rename = "dense_turn")]
    DenseTurin,
    #[serde(rename = "test")]
    Test,
}

#[derive(Debug, Deserialize)]
pub struct Patch {
    base: PathBuf,
    diff: PathBuf,
}

impl Patch {
    pub fn base(&self) -> &Path {
        self.base.as_ref()
    }

    pub fn diff(&self) -> &Path {
        self.diff.as_ref()
    }
}

#[derive(Debug, Deserialize)]
pub struct App {
    cpu: Cpu,
    firmware_version: String,
    patch: Option<Patch>,
    size: u32,
    board: String,
    blobs: Vec<PathBuf>,
}

impl App {
    pub fn try_from_str(data: &str) -> Result<App> {
        let app = toml::from_str(data)?;
        Ok(app)
    }

    pub fn name(&self) -> String {
        let cpu = format!("{cpu:?}", cpu = self.cpu);
        format!(
            "{cpu}-{board}-{fwvers}",
            cpu = cpu.to_ascii_lowercase(),
            board = self.board,
            fwvers = self.firmware_version,
        )
    }

    fn cpu_dir(&self) -> &Path {
        match self.cpu {
            Cpu::Rome => Path::new("SSP"),  // Starship
            Cpu::Milan => Path::new("GN"),  // Genesis
            Cpu::Genoa => Path::new("RS"),  // Rolling Stones
            Cpu::Turin => Path::new("BRH"), // Breithorn
            Cpu::DenseTurin => Path::new("BRH"),
            Cpu::Test => Path::new("data"), // Dummy test data
        }
    }

    pub fn blob_path(&self, base: &Path) -> PathBuf {
        let mut path = base.to_path_buf();
        path.push(self.cpu_dir());
        path.push(&self.firmware_version);
        path
    }

    pub fn blobs(&self) -> &[PathBuf] {
        self.blobs.as_ref()
    }

    pub fn size(&self) -> String {
        format!("{size}MiB", size = self.size)
    }

    pub fn patch(&self) -> Option<&Patch> {
        self.patch.as_ref()
    }
}

pub fn try_from_file(app: &Path) -> Result<App> {
    eprintln!("reading from {app:?}");
    let data = fs::read(app)?;
    //eprintln!("data: {data:?}");
    let data = match std::str::from_utf8(&data) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error {e:?}");
            return Err(e.into());
        }
    };
    App::try_from_str(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_app() {
        let data = r#"
        cpu = 'turin'
        board = 'ruby'
        size = 16
        firmware_version = '1.0.0.3-p1'
        blobs = [
            'a',
            'b',
            'c',
        ]"#;
        let maybe = App::try_from_str(data);
        assert!(maybe.is_ok());
        let app = maybe.unwrap();
        assert_eq!(app.cpu, Cpu::Turin);
        assert_eq!(app.firmware_version, "1.0.0.3-p1");
        assert_eq!(app.size, 16);
        assert_eq!(app.board, "ruby");
        assert_eq!(app.blobs, [PathBuf::from("a"), "b".into(), "c".into()]);
        assert_eq!(
            app.blob_path(Path::new("/fw")).to_str(),
            Some("/fw/BRH/1.0.0.3-p1")
        );
    }
}
