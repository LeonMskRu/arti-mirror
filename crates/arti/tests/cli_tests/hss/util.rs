use std::{path::Path, process::Output};

use assert_cmd::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::util::create_state_dir_entry;

/// Path to a test specific configuration.
const CFG_PATH: &str = "./tests/testcases/hss-extra/conf/hss.toml";

/// A struct that represents the subcommand `hss ctor-migrate`.
#[derive(Debug)]
pub struct CTorMigrateCmd {
    /// The temporary directory representing the state directory.
    ///
    /// NOTE: Although this field is not used directly, it must be retained to prevent the
    /// temporary directory from being dropped prematurely.
    #[allow(dead_code)]
    state_dir: TempDir,
    /// The file path to the state directory.
    state_dir_path: String,
}

impl CTorMigrateCmd {
    /// A fresh instance of `CTorMigrateCmd`.
    pub fn new() -> Self {
        let state_dir = TempDir::new().unwrap();
        let state_dir_path = state_dir.path().to_path_buf();
        let state_dir_path = state_dir_path.to_string_lossy().to_string();
        Self {
            state_dir,
            state_dir_path,
        }
    }

    /// Execute the command and return its output as an [`Output`].
    pub fn output(&self) -> std::io::Result<Output> {
        let mut cmd = Command::cargo_bin("arti").unwrap();

        let opt = create_state_dir_entry(&self.state_dir_path);
        cmd.args([
            "-c",
            CFG_PATH,
            "-o",
            &opt,
            "hss",
            "-n",
            "allium-cepa",
            "ctor-migrate",
            "-b",
        ]);

        cmd.output()
    }

    /// Check whether the state directory is empty.
    pub fn is_state_dir_empty(&self) -> bool {
        self.state_dir_entries().is_empty()
    }

    /// Check wheter the state directory contains a long-term identity key.
    pub fn state_dir_contains_id(&self) -> bool {
        let expected_path = Path::new(&self.state_dir_path)
            .join("keystore")
            .join("hss")
            .join("allium-cepa")
            .join("ks_hs_id.ed25519_expanded_private");

        self.state_dir_entries().iter().any(|e| {
            if let Ok(entry) = e {
                entry.path() == expected_path
            } else {
                false
            }
        })
    }

    /// Returns a vector containing all entries in the state directory.
    ///
    /// Each element is a `Result`, with `Err` indicating an I/O error encountered
    /// while reading an entry.
    fn state_dir_entries(&self) -> Vec<Result<walkdir::DirEntry, walkdir::Error>> {
        WalkDir::new(&self.state_dir_path)
            .into_iter()
            .skip(1)
            .collect()
    }
}
