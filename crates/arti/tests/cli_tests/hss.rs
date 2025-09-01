//! Integration test suite for Arti hss.
//!
//! Testing certain `hss` subcommands involves deleting and creating files, which requires the use
//! of a temporary directory. Due to this and other considerations, the `assert_cmd` crate is used to
//! test these subcommands instead of the preferred `trycmd` crate (see [README](../README.md)).
//!
//! Test data for this suite is stored in the `hss-extra` directory.

use crate::hss::util::{
    ARTI_KEYSTORE_POPULATION, CTorMigrateCmd, EXPECTED_ID_KEY_PATH,
    EXPECTED_UNRECOGNIZED_KEYSTORE_ENTRY, HSS_DIR_PATH, IPTS_DIR_PATH, KEYSTORE_DIR_PATH,
    SERVICE_DIR_PATH, UNRECOGNIZED_DIR_PATH, UNRECOGNIZED_SERVICE_ID_PATH,
    UNRECOGNIZED_SERVICE_PATH,
};

mod util;

#[test]
fn migrate_with_empty_arti_keystore_succeede() {
    let cmd = CTorMigrateCmd::new();
    assert!(cmd.is_state_dir_empty());
    assert!(cmd.output().unwrap().status.success());
    assert!(cmd.state_dir_contains_only(&[
        EXPECTED_ID_KEY_PATH,
        KEYSTORE_DIR_PATH,
        HSS_DIR_PATH,
        SERVICE_DIR_PATH
    ]));
}

#[test]
fn migrate_with_full_arti_keystore_succeede_with_batch_flag_activated() {
    let cmd = CTorMigrateCmd::new();
    assert!(cmd.is_state_dir_empty());
    cmd.populate_state_dir();
    assert!(cmd.state_dir_contains_only(ARTI_KEYSTORE_POPULATION));
    assert!(cmd.output().unwrap().status.success());
    // `ctor-migrate` substitutes the long-term ID key, removes all recognized entries,
    // and ignores unrecognized entries and paths.
    assert!(cmd.state_dir_contains_only(&[
        EXPECTED_ID_KEY_PATH,
        KEYSTORE_DIR_PATH,
        HSS_DIR_PATH,
        SERVICE_DIR_PATH,
        EXPECTED_UNRECOGNIZED_KEYSTORE_ENTRY,
        IPTS_DIR_PATH,
        UNRECOGNIZED_DIR_PATH,
        UNRECOGNIZED_SERVICE_PATH,
        UNRECOGNIZED_SERVICE_ID_PATH
    ]));
}
