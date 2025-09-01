use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
    str::FromStr,
};

use assert_cmd::Command;
use std::io;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::util::create_state_dir_entry;

/// Path to a test specific configuration that provides a full Arti native keystore.
pub const CFG_PATH: &str = "./tests/testcases/hss-extra/conf/hss.toml";

/// Path to a test specific configuration that provides a full Arti native keystore and a full CTor
/// keystore.
pub const CFG_CTOR_PATH: &str = "./tests/testcases/hss-extra/conf/hss-ctor.toml";

/// Path to a fully populated Arti native keystore.
const KEYSTORE_PATH: &str = "./tests/testcases/hss-extra/hss.in/local/state-dir";

/// Path to the long-term ID key, relative to the state directory.
pub const EXPECTED_ID_KEY_PATH: &str = "keystore/hss/allium-cepa/ks_hs_id.ed25519_expanded_private";

/// Path to the keystore directory, relative to the state directory.
pub const KEYSTORE_DIR_PATH: &str = "keystore";

/// Path to the keystore directory, relative to the state directory.
pub const HSS_DIR_PATH: &str = "keystore/hss";

/// Path to the keystore directory, relative to the state directory.
pub const SERVICE_DIR_PATH: &str = "keystore/hss/allium-cepa";

/// Path to an unrecognized keystore entry, relative to the state directory.
pub const EXPECTED_UNRECOGNIZED_KEYSTORE_ENTRY: &str = "keystore/hss/allium-cepa/herba-spontanea";

/// Path to ipts directory, relative to the state directory.
pub const IPTS_DIR_PATH: &str = "keystore/hss/allium-cepa/ipts";

/// A part of an unrecognized path, relative to the state directory.
pub const UNRECOGNIZED_DIR_PATH: &str = "keystore/opus-abusivum";

/// A part of an unrecognized path, relative to the state directory.
pub const UNRECOGNIZED_SERVICE_PATH: &str = "keystore/opus-abusivum/herba-spontanea";

/// Unrecognized path, relative to the state directory.
pub const UNRECOGNIZED_SERVICE_ID_PATH: &str =
    "keystore/opus-abusivum/herba-spontanea/ks_hs_id.ed25519_expanded_private";

/// A collection of every path present in the default state directory.
pub const ARTI_KEYSTORE_POPULATION: &[&str] = &[
    KEYSTORE_DIR_PATH,
    HSS_DIR_PATH,
    SERVICE_DIR_PATH,
    EXPECTED_ID_KEY_PATH,
    EXPECTED_UNRECOGNIZED_KEYSTORE_ENTRY,
    "keystore/hss/allium-cepa/ks_hs_blind_id+20326_1440_43200.ed25519_expanded_private",
    "keystore/hss/allium-cepa/ks_hs_blind_id+20327_1440_43200.ed25519_expanded_private",
    IPTS_DIR_PATH,
    "keystore/hss/allium-cepa/ipts/k_sid+ce8514e2fe016e4705b064f2226a7628f4226e9a15d28607112e4eac3b3a012f.ed25519_private",
    "keystore/hss/allium-cepa/ipts/k_sid+2a6054c3432b880b76cf379f66daf1a34c88693efed5e85bd90507a1fea231d7.ed25519_private",
    "keystore/hss/allium-cepa/ipts/k_sid+84a3a863484ff521081ee8e6e48a6129d0c83bef89fe294a5dda6f782b43dec8.ed25519_private",
    "keystore/hss/allium-cepa/ipts/k_hss_ntor+ce8514e2fe016e4705b064f2226a7628f4226e9a15d28607112e4eac3b3a012f.x25519_private",
    "keystore/hss/allium-cepa/ipts/k_hss_ntor+84a3a863484ff521081ee8e6e48a6129d0c83bef89fe294a5dda6f782b43dec8.x25519_private",
    "keystore/hss/allium-cepa/ipts/k_hss_ntor+2a6054c3432b880b76cf379f66daf1a34c88693efed5e85bd90507a1fea231d7.x25519_private",
    UNRECOGNIZED_DIR_PATH,
    UNRECOGNIZED_SERVICE_PATH,
    UNRECOGNIZED_SERVICE_ID_PATH,
];

/// A struct that represents the subcommand `hss ctor-migrate`.
#[derive(Debug, amplify::Getters)]
pub struct CTorMigrateCmd {
    /// The temporary directory representing the state directory.
    ///
    /// NOTE: Although this field is not used directly, it must be retained to prevent the
    /// temporary directory from being dropped prematurely.
    #[allow(dead_code)]
    #[getter(skip)]
    state_dir: TempDir,
    /// The file path to the state directory.
    state_dir_path: PathBuf,
    /// Nickname of the service to be migrated, defaults to `"allium-cepa"`.
    #[getter(skip)]
    nickname: String,
    /// Configuration to the configuration file that will be used, defaults
    /// to `CFG_CTOR_PATH`.
    #[getter(skip)]
    config: String,
}

impl CTorMigrateCmd {
    /// A fresh instance of `CTorMigrateCmd`.
    pub fn new() -> Self {
        let state_dir = TempDir::new().unwrap();
        let state_dir_path = state_dir.path().to_path_buf();
        Self {
            state_dir,
            state_dir_path,
            nickname: "allium-cepa".to_string(),
            config: CFG_CTOR_PATH.to_string(),
        }
    }

    /// Execute the command and return its output as an [`Output`].
    pub fn output(&self) -> std::io::Result<Output> {
        let mut cmd = Command::cargo_bin("arti").unwrap();

        let opt = create_state_dir_entry(self.state_dir_path.to_string_lossy().as_ref());
        cmd.args([
            "-c",
            &self.config,
            "-o",
            &opt,
            "hss",
            "-n",
            &self.nickname,
            "ctor-migrate",
            "-b",
        ]);

        cmd.output()
    }

    /// Populates the temporary state directory with the files from the default state directory.
    pub fn populate_state_dir(&self) {
        let keystore_path = PathBuf::from_str(KEYSTORE_PATH).unwrap();
        Self::clone_dir(&keystore_path, &self.state_dir_path);
    }

    /// Recursively clones the entire contents of the directory `source` into the
    /// directory `destination`.
    ///
    /// This function does not check whether `source` and `destination` exist,
    /// whether they are directories, or perform any other validation.
    fn clone_dir(source: &Path, destination: &Path) {
        let entries = fs::read_dir(source).unwrap();
        for entry in entries {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let source_path = entry.path();
            let file_name = source_path.file_name().unwrap();
            let destination_path = destination.join(file_name);
            if file_type.is_dir() {
                if let Err(e) = fs::create_dir(&destination_path) {
                    if e.kind() != io::ErrorKind::AlreadyExists {
                        panic!("{}", e)
                    }
                };
                Self::clone_dir(&source_path, &destination_path);
            } else if file_type.is_file() {
                fs::copy(&source_path, &destination_path).unwrap();
            }
        }
    }

    /// Check whether the state directory is empty.
    pub fn is_state_dir_empty(&self) -> bool {
        self.state_dir_entries().is_empty()
    }

    /// Check whether the state directory contains only the provided entries.
    pub fn state_dir_contains_only(&self, expected_entries: &[&str]) -> bool {
        let state_dir_entries = self.state_dir_entries();
        let entries: Vec<_> = state_dir_entries
            .iter()
            .map(|res| {
                let entry = res.as_ref().unwrap();
                entry.path().to_string_lossy().to_string()
            })
            .collect();
        if entries.len() != expected_entries.len() {
            return false;
        }
        for entry in expected_entries {
            let path = format!(
                "{}/{}",
                self.state_dir_path.to_string_lossy().as_ref(),
                entry
            );
            if !entries.contains(&path) {
                return false;
            }
        }
        true
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

    /// Setter for the field `nickname`
    pub fn set_nickname(&mut self, nickname: String) {
        self.nickname = nickname;
    }

    /// Setter for the field `config`
    pub fn set_config(&mut self, config: String) {
        self.config = config;
    }
}

/// A struct that represents the subcommand `hss onion-address`.
#[derive(Debug, Clone, Default, Eq, PartialEq, derive_builder::Builder)]
pub struct OnionAddressCmd {
    /// Path to the configuration file supplied as the value of the `-c` flag.
    config_path: String,
    /// Optional path to a state directory.
    /// If `Some`, passed as the value to the `-o` flag.
    #[builder(default)]
    state_directory: Option<String>,
}

impl OnionAddressCmd {
    /// Execute the command and return its output as an [`Output`].
    pub fn output(&self) -> std::io::Result<Output> {
        let mut cmd = Command::cargo_bin("arti").unwrap();
        cmd.args(["-c", &self.config_path]);
        if let Some(state_directory) = &self.state_directory {
            let opt = create_state_dir_entry(state_directory);
            cmd.args(["-o", &opt]);
        }
        cmd.args(["hss", "-n", "allium-cepa", "onion-address"]);

        cmd.output()
    }
}
