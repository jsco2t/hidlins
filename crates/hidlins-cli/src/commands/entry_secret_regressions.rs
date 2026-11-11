//! Observe generated entry-password ownership at its actual deallocation.

#[path = "../../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;

use clap::Parser;

use crate::cli::{Cli, Command, EntryVerb};
use allocation_observer::DeallocationWatch;

#[test]
fn generated_entry_password_owner_erases_its_allocation() {
    let cli = Cli::try_parse_from([
        "hidlins",
        "entry",
        "add",
        "--vault",
        "v",
        "--title",
        "synthetic",
        "--generate",
    ])
    .unwrap();
    let Some(Command::Entry(args)) = cli.command else {
        unreachable!();
    };
    let Some(EntryVerb::Add(args)) = args.verb else {
        unreachable!();
    };
    let value = super::resolve_add_password(&args).unwrap().unwrap();
    assert!(!value.is_empty());
    let watch = DeallocationWatch::new(value.as_bytes());
    drop(value);
    watch.assert_erased();
}
