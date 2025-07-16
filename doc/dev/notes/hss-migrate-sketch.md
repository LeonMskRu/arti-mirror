# Draft for `arti hss ctor-migrate` CLI tool

## Summary

This tool will provide a C Tor to Arti key migration tool, which will enable
onion service operators to seamlessly migrate from C Tor to Arti (and, maybe,
vice versa).


## A flexible description

### CLI

The command will be:
```bash
arti hss ctor-migrate --config <ARTI_CONFIG> \
    --nickname <HS_NICK> \
    --from <CTOR_KEYSTORE_ID> \
    [--to <TARGET_KEYSTORE_ID>]
```
Where `CTOR_KEYSTORE_ID` is the keystore ID of the C Tor keystore to migrate, as
configured in the `<ARTI_CONFIG>`, under the `[storage.keystore.ctor.services.<HS_NICK>]`
section.

`<TARGET_KEYSTORE_ID>` should be the keystore ID of one of the keystores configured
in Arti's TOML config. By default, `TARGET_KEYSTORE_ID` is set to `arti` (Arti's default,
native primary keystore). If the user specifies a keystore ID not associated with
any of the configured keystores, the output will provide instructions on how to include
it in the configuration file, such as: `Add this line to your configuration file: <LINE>`.

> Note: the keystore ID of Arti's primary keystore is currently hard-coded to "arti",
and is not configurable (#1106). Until #1106 is addressed, users won't have any use
for the `--to` flag (it only exists for future-proofing reasons).

The migration tool will conduct a preliminary check to ensure the keys being migrated
don’t already have a corresponding entry in the target. If any do, the migration will
be aborted. This behavior could be controlled by an additional flag: `force`/`batch`.
This would determine whether the existing keys should be overwritten.
An alternative solution could be to prompt the operator.

The migration should only be executed when both the C Tor service the keys originated
from and the target arti service are not running.

If a TOCTOU race occurs, meaning one of the C Tor keys we’re migrating disappears or
another process writes one of the corresponding keys in the Arti keystore (and our
preliminary check has passed), the migration will be aborted.

`ctor-migrate` will be idempotent, meaning that if it’s run multiple times with the
same configuration after the migration is complete, the migration won’t be performed
again, and a message such as "already migrated" will be displayed.

The default behavior will be to leave the original CTor keystore intact, this will
also facilitate an eventual backward migration. This behavior could be changed
unsing a flag.

The keys in the CTor keystore are expected to be valid. Therefore, the command will
produce an error and will not proceed with the action if an invalid key is
encountered.


### Implemantation Detail

A specialized `KeyMgr` method should be added, so that at the `arti::subcommands::hss`
level a single call will be sufficient to obtain the result. This will abstract away
migrate logic as much as possible from `arti::subcommands::hss`.
The method will have signature:
```rust
fn migrate(&self, config: KeystoreMigrationConfig) -> tor_keymgr::Result<()>
```

Where `KeystoreMigrationConfig` is a wrapper around the two pertinent `KeystoreId`s:

```rust
KeystoreMigrationConfig
{
    to: KeytoreId,
    from: KeystoreId,
    /* ... */
}
```

Some issues could arise during the removal phase, as the components currently available
to remove the keys do not work with the CTor keystore (`Keystore::remove_unchecked`).
The existing interface could be modified to achieve the desired result; in that case,
`arti keys-raw remove-by-id` would need slight reworking, or a new interface could be
added: `Keystore::remove_ctor_entry`, this could returned the removed entry, given
that the keys in the CTor keystore are supposed to be valid.


### Notes

The design is neither complete nor final. In fact, the purpose of this note is
to gather feedback and insights.

This note is part of the 2025 GSoC proposal "Onion Service Support Tooling for
Arti" ([link](https://gitlab.torproject.org/tpo/team/-/wikis/GSoC#2-project-onion-service-support-tooling-for-arti)).

Related [milestone](https://gitlab.torproject.org/tpo/core/arti/-/milestones/22#tab-issues),
Related [issue](https://gitlab.torproject.org/tpo/core/arti/-/issues/2072).
