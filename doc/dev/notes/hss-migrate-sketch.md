# Draft for `arti hss ctor-migrate` CLI tool

## Summary

This tool will provide a C Tor to Arti key migration tool, which will enable
onion service operators to seamlessly migrate from C Tor to Arti (and, maybe,
vice versa).


## A flexible description

### CLI

The command will be:
```bash
arti hss ctor-migrate --config <ARTI_CONFIG> --nickname <HS_NICK> --from <CTOR_KEYSTORE_ID> [--to <TARGET_KEYSTORE_ID>]
```
Where `CTOR_KEYSTORE_ID` is the keystore ID of the C Tor keystore to migrate, as
configured in the `<ARTI_CONFIG>`, under the `[storage.keystore.ctor.services.<HS_NICK>]`
section.

A possible variation could involve using a `PATH_TO_CTOR` instead of a `CTOR_KEYSTORE_ID`,
allowing the operator to perform the migration without needing to alter the configuration
file specifically for this operation.

`<TARGET_KEYSTORE_ID>` could represent either a configured or a non-configured keystore.
If the keystore doesn't exist, a new one could be created. In that case, the output would
provide instructions on how to include it in the configuration file, such as: `Add this
line to your configuration file: <LINE>`.

If the keystore already exists, its behavior could be controlled by an additional flag:
`force`/`batch`. This would determine whether the existing keys should be overwritten.
An alternative solution could be to prompt the operator.

The default behavior may be to remove the CTor keystore once the migration is complete,
in order to avoid key duplication. Alternatively, it could leave the CTor keystore
intact to facilitate a backward migration. The previous considerations regarding
flags/prompts apply here as well.

The keys in the CTor keystore are expected to be valid. Therefore, the command will
produce an error and will not proceed with the action if an invalid key is
encountered.


### Implemantation Detail

A specialized `KeyMgr` method should be added, so that at the `arti::subcommands::hss`
level a single call will be sufficient to obtain the result. This will abstract away
migrate logic as much as possible from `arti::subcommands::hss`.
The method will have signature:
```rust
fn migrate(&self, to: KeystoreId, from: KeystoreId) -> tor_keymgr::Result<Information>
```
where `Information` is a placeholder for something that could be useful to return.

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
