use hidlins_genpw::{CharSet, GenError, PassphraseBuilder, PasswordBuilder};

#[test]
fn deterministic_draws_preserve_reserve_fill_and_shuffle() {
    let password = PasswordBuilder::new()
        .generate_from_bytes(&[0; 512])
        .unwrap();
    assert_eq!(password.as_str(), format!("A0!{}", "a".repeat(17)));
    let password = PasswordBuilder::new()
        .length(1)
        .classes(CharSet {
            digits: true,
            ..CharSet::NONE
        })
        .generate_from_bytes(&[9, 0, 0, 0])
        .unwrap();
    assert_eq!(password.as_str(), "9");
}

#[test]
fn exact_word_choices_and_separators_remain_unchanged() {
    let bytes: Vec<u8> = [0u32, 1, 2]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for separator in ["", "-", " :: ", " 🔒 "] {
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

#[test]
fn invalid_configuration_and_entropy_exhaustion_keep_typed_errors() {
    assert!(matches!(
        PasswordBuilder::new().length(0).generate(),
        Err(GenError::InvalidLength)
    ));
    assert!(matches!(
        PassphraseBuilder::new().words(0).generate(),
        Err(GenError::InvalidWordCount)
    ));
    assert!(matches!(
        PasswordBuilder::new().generate_from_bytes(&[]),
        Err(GenError::Csprng(_))
    ));
    assert!(matches!(
        PassphraseBuilder::new().generate_from_bytes(&[]),
        Err(GenError::Csprng(_))
    ));
}
