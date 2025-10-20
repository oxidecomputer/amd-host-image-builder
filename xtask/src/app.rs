// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{Context, Result, anyhow};
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
    pub fn with_root(self, root: Option<&Path>) -> Patch {
        Patch {
            base: root_path(root, self.base),
            diff: root_path(root, self.diff),
        }
    }

    pub fn base(&self) -> &Path {
        self.base.as_ref()
    }

    pub fn diff(&self) -> &Path {
        self.diff.as_ref()
    }
}

#[derive(Deserialize)]
struct AppRaw {
    base: Option<PathBuf>,
    cpu: Option<Cpu>,
    firmware_version: Option<String>,
    patch: Option<Patch>,
    size: Option<u32>,
    board: Option<String>,
    blobs: Option<Vec<PathBuf>>,
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

fn root_path(root: Option<&Path>, p: PathBuf) -> PathBuf {
    if let Some(root) = root { root.join(p) } else { p }
}

impl App {
    pub fn try_from_str(root: Option<&Path>, data: &str) -> Result<App> {
        let app_raw: AppRaw = toml::from_str(data)?;
        let app = if let Some(base) = app_raw.base {
            let base = try_from_file(root, &root_path(root, base))
                .context("Failed to parse base App config")?;
            App {
                cpu: app_raw.cpu.unwrap_or(base.cpu),
                firmware_version: app_raw
                    .firmware_version
                    .unwrap_or(base.firmware_version),
                patch: if app_raw.patch.is_some() {
                    app_raw.patch
                } else {
                    base.patch
                }
                .map(|p| p.with_root(root)),
                size: app_raw.size.unwrap_or(base.size),
                board: app_raw.board.unwrap_or(base.board),
                blobs: app_raw.blobs.unwrap_or(base.blobs),
            }
        } else {
            App {
                cpu: app_raw.cpu.ok_or_else(|| anyhow!("'cpu' missing"))?,
                firmware_version: app_raw
                    .firmware_version
                    .ok_or_else(|| anyhow!("'firmware_version' missing"))?,
                patch: app_raw.patch.map(|p| p.with_root(root)),
                size: app_raw.size.ok_or_else(|| anyhow!("'size' missing"))?,
                board: app_raw
                    .board
                    .ok_or_else(|| anyhow!("'board' missing"))?,
                blobs: app_raw
                    .blobs
                    .ok_or_else(|| anyhow!("'blobs' missing"))?,
            }
        };
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

pub fn try_from_file(root: Option<&Path>, app: &Path) -> Result<App> {
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
    App::try_from_str(root, data)
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
        let maybe = App::try_from_str(None, data);
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

    #[test]
    fn inherit_app() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let base_data = r#"
        cpu = 'turin'
        board = 'ruby'
        size = 16
        firmware_version = '1.0.0.3-p1'
        blobs = [
            'a',
            'b',
            'c',
        ]"#;
        assert!(App::try_from_str(None, base_data).is_ok());
        let mut base_app = NamedTempFile::new().unwrap();
        writeln!(base_app, "{base_data}").unwrap();
        base_app.flush().unwrap();

        // Inherit from base but replace 'board' value
        let data = format!(
            r#"
        base = '{}'
        board = 'cosmo'
        [patch]
        base = 'base.json5'
        diff = 'diff.patch'
        "#,
            base_app.path().display()
        );
        let maybe = App::try_from_str(None, &data);
        assert!(maybe.is_ok());
        let app = maybe.unwrap();
        assert_eq!(app.cpu, Cpu::Turin);
        assert_eq!(app.firmware_version, "1.0.0.3-p1");
        assert_eq!(app.size, 16);
        assert_eq!(app.board, "cosmo");
        assert_eq!(app.blobs, [PathBuf::from("a"), "b".into(), "c".into()]);
        assert_eq!(
            app.blob_path(Path::new("/fw")).to_str(),
            Some("/fw/BRH/1.0.0.3-p1")
        );
        assert!(app.patch.is_some());
        let patch = app.patch.unwrap();
        assert_eq!(patch.base, PathBuf::from("base.json5"));
        assert_eq!(patch.diff, PathBuf::from("diff.patch"));
    }

    #[test]
    fn patch_root() {
        let data = r#"
        cpu = 'test'
        board = 'test'
        size = 16
        firmware_version = 'test'
        blobs = []
        [patch]
        base = '/base.json5'
        diff = 'diff.patch'
        "#;
        let maybe = App::try_from_str(Some(&Path::new("/configs")), data);
        assert!(maybe.is_ok());
        let app = maybe.unwrap();
        assert!(app.patch.is_some());
        let patch = app.patch.unwrap();
        // Base path was absolute so should remain unchanged
        assert_eq!(patch.base, PathBuf::from("/base.json5"));
        assert_eq!(patch.diff, PathBuf::from("/configs/diff.patch"));
    }
}
