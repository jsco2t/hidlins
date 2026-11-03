#[path = "common/scratch_allocator.rs"]
mod scratch_allocator;

use hidlins_genpw::{CharSet, GenError, PassphraseBuilder, PasswordBuilder};
use scratch_allocator::assert_erased;

const COUNT: usize = 17;

fn password_builder() -> PasswordBuilder {
    PasswordBuilder::new().length(COUNT).classes(CharSet {
        lowercase: true,
        ..CharSet::NONE
    })
}

#[test]
fn password_scratch_erases_after_success() {
    let expected = "a".repeat(COUNT);
    // Union/class alphabet buffers are104 bytes; only output scratch is68.
    assert_erased(
        [
            (COUNT * size_of::<char>(), COUNT * size_of::<char>()),
            (0, 0),
        ],
        || {
            let result = password_builder().generate_from_bytes(&[0; 512]).unwrap();
            assert_eq!(result.as_str(), expected);
        },
    );
}

#[test]
fn password_scratch_erases_after_partial_failure() {
    // Two complete draws initialize exactly two chars before the next fails.
    assert_erased(
        [(COUNT * size_of::<char>(), 2 * size_of::<char>()), (0, 0)],
        || {
            assert!(matches!(
                password_builder().generate_from_bytes(&[0; 8]),
                Err(GenError::Csprng(_))
            ));
        },
    );
}

fn selection_layouts(initialized: usize) -> [(usize, usize); 2] {
    // Original references and erased-index storage both identify the selected
    // words. Only initialized elements are read, including on partial failure.
    [
        (COUNT * size_of::<&str>(), initialized * size_of::<&str>()),
        (COUNT * size_of::<usize>(), initialized * size_of::<usize>()),
    ]
}

#[test]
fn passphrase_selection_erases_after_success() {
    let bytes: Vec<u8> = (0..COUNT).flat_map(|_| 1u32.to_le_bytes()).collect();
    let expected = vec!["abdomen"; COUNT].join("|");
    let builder = PassphraseBuilder::new().words(COUNT).separator("|");
    assert_erased(selection_layouts(COUNT), || {
        let result = builder.generate_from_bytes(&bytes).unwrap();
        assert_eq!(result.as_str(), expected);
    });
}

#[test]
fn passphrase_selection_erases_after_partial_failure() {
    let bytes: Vec<u8> = (0..2).flat_map(|_| 1u32.to_le_bytes()).collect();
    let builder = PassphraseBuilder::new().words(COUNT);
    assert_erased(selection_layouts(2), || {
        assert!(matches!(
            builder.generate_from_bytes(&bytes),
            Err(GenError::Csprng(_))
        ));
    });
}

#[test]
fn deterministic_output_keeps_words_order_and_verbatim_separators() {
    let bytes: Vec<u8> = [0u32, 1, 2]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for separator in ["", "|", " :: ", " 🔒 "] {
        let result = PassphraseBuilder::new()
            .words(3)
            .separator(separator)
            .generate_from_bytes(&bytes)
            .unwrap();
        assert_eq!(
            result.as_str(),
            ["abacus", "abdomen", "abdominal"].join(separator)
        );
    }
}
