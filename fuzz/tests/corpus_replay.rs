use std::{fs, path::Path};

fn replay(directory: &str, harness: fn(&[u8])) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(directory);
    let mut entries: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()))
        .map(|entry| entry.expect("read corpus entry").path())
        .collect();
    entries.sort();
    assert!(
        !entries.is_empty(),
        "corpus {} must not be empty",
        root.display()
    );
    for path in entries {
        let input = fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        harness(&input);
    }
}

#[test]
fn lns_fuzz_corpus_001_protocol_decoder_replays_on_stable() {
    replay("local_protocol", hidlins_local_sync_fuzz::protocol_input);
}

#[test]
fn lns_fuzz_corpus_002_address_policy_replays_on_stable() {
    replay("local_address", hidlins_local_sync_fuzz::address_input);
}

#[test]
fn lns_fuzz_corpus_003_pairing_state_replays_on_stable() {
    replay("pairing_state", hidlins_local_sync_fuzz::state_input);
}
