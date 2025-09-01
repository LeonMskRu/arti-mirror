//! Integration test suite for Arti hss.
//!
//! Testing certain `hss` subcommands involves deleting and creating files, which requires the use
//! of a temporary directory. Due to this and other considerations, the `assert_cmd` crate is used to
//! test these subcommands instead of the preferred `trycmd` crate (see [README](../README.md)).
//!
//! Test data for this suite is stored in the `hss-extra` directory.

use crate::hss::util::{
    ARTI_KEYSTORE_POPULATION, CFG_CTOR_PATH, CFG_PATH, CTorMigrateCmd, EXPECTED_ID_KEY_PATH,
    EXPECTED_UNRECOGNIZED_KEYSTORE_ENTRY, HSS_DIR_PATH, IPTS_DIR_PATH, KEYSTORE_DIR_PATH,
    OnionAddressCmdBuilder, SERVICE_DIR_PATH, UNRECOGNIZED_DIR_PATH, UNRECOGNIZED_SERVICE_ID_PATH,
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
    let migrate_cmd = CTorMigrateCmd::new();
    assert!(migrate_cmd.is_state_dir_empty());

    // Since the state directory is currently empty and the configuration provides
    // a functional CTor keystore, `ctor_keystore_onion_address` holds the onion
    // address associated with the CTor keystore's identity key.
    let onion_address_cmd = OnionAddressCmdBuilder::default()
        .config_path(CFG_CTOR_PATH.to_string())
        .state_directory(Some(
            migrate_cmd.state_dir_path().to_string_lossy().to_string(),
        ))
        .build()
        .unwrap();
    let ctor_keystore_onion_address =
        String::from_utf8(onion_address_cmd.output().unwrap().stdout).unwrap();

    migrate_cmd.populate_state_dir();

    // With the state directory populated and the configuration (lacking a CTor keystore) overridden by a
    // command-line flag that provides a functional, fully populated Arti native keystore, the resulting
    // onion address is derived from the Arti keystore's identity key.
    let onion_address_cmd = OnionAddressCmdBuilder::default()
        .config_path(CFG_PATH.to_string())
        .state_directory(Some(
            migrate_cmd.state_dir_path().to_string_lossy().to_string(),
        ))
        .build()
        .unwrap();
    let arti_keystore_onion_address =
        String::from_utf8(onion_address_cmd.output().unwrap().stdout).unwrap();

    // The two onion-addresses are different.
    assert_ne!(ctor_keystore_onion_address, arti_keystore_onion_address);

    assert!(migrate_cmd.state_dir_contains_only(ARTI_KEYSTORE_POPULATION));
    assert!(migrate_cmd.output().unwrap().status.success());
    // `ctor-migrate` substitutes the long-term ID key, removes all recognized entries,
    // and ignores unrecognized entries and paths.
    assert!(migrate_cmd.state_dir_contains_only(&[
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

    // The migration has completed: the CTor identity key has been converted into an
    // Arti native identity key. Since no CTor keystore is provided, the resulting
    // onion address is derived from the Arti native keystore, which holds the same
    // identity key as the original CTor keystore.
    let onion_address_cmd = OnionAddressCmdBuilder::default()
        .config_path(CFG_PATH.to_string())
        .state_directory(Some(
            migrate_cmd.state_dir_path().to_string_lossy().to_string(),
        ))
        .build()
        .unwrap();
    let arti_keystore_onion_address =
        String::from_utf8(onion_address_cmd.output().unwrap().stdout).unwrap();

    // The onion addresses are now the same because they are derived from the same
    // identity key, which exists in different formats across the two keystores.
    assert_eq!(ctor_keystore_onion_address, arti_keystore_onion_address)
}

#[test]
fn ctor_migrate_is_idempotent() {
    let cmd = CTorMigrateCmd::new();
    assert!(cmd.is_state_dir_empty());
    assert!(cmd.output().unwrap().status.success());
    assert!(cmd.state_dir_contains_only(&[
        EXPECTED_ID_KEY_PATH,
        KEYSTORE_DIR_PATH,
        HSS_DIR_PATH,
        SERVICE_DIR_PATH
    ]));
    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(cmd.output().unwrap().stderr).unwrap();
    assert!(error.contains("error: Service allium-cepa was already migrated."))
}

#[test]
fn ctor_migrate_fails_if_applied_to_unregistered_service() {
    let mut cmd = CTorMigrateCmd::new();
    assert!(cmd.is_state_dir_empty());
    cmd.set_nickname("unregistered".to_string());
    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(cmd.output().unwrap().stderr).unwrap();
    assert!(error.contains("error: The service identified using `--nickname unregistered` is not configured with any recognized CTor keystore."))
}
