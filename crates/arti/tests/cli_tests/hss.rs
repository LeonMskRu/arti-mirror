//! Integration test suite for Arti hss.
//!
//! Testing certain `hss` subcommands involves deleting and creating files, which requires the use
//! of a temporary directory. Due to this and other considerations, the `assert_cmd` crate is used to
//! test these subcommands instead of the preferred `trycmd` crate (see [README](../README.md)).
//!
//! Test data for this suite is stored in the `hss-extra` directory.

use crate::hss::util::CTorMigrateCmd;

mod util;

#[test]
fn migrate_with_empty_arti_keystore_succeede() {
    let cmd = CTorMigrateCmd::new();
    assert!(cmd.is_state_dir_empty());
    assert!(cmd.output().unwrap().status.success());
    assert!(cmd.state_dir_contains_id());
}
