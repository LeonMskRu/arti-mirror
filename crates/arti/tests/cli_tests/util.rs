//! Utilities for integration testing of CLI subcommands.

use std::process::Output;

/// Due to the "destroy" policy of some service configurations,
/// in some of the tests stderr is not empty; instead, it contains
/// a log message.
/// This function asserts that only this message is present in
/// the stderr channel.
pub fn assert_log_message(output: Output) {
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "arti:\u{1b}[33m WARN\u{1b}[0m \u{1b}[2mtor_hsrproxy::config\u{1b}[0m\u{1b}[2m:\u{1b}[0m Onion service is not configured to accept any connections.\n"
    );
}
