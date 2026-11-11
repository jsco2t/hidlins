//! Pin correct compound-word behavior before correcting the CLI test oracle.
mod common;

use hidlins_genpw::PassphraseBuilder;

#[test]
fn all_eff_compounds_remain_words_with_default_and_unambiguous_separators() {
    let raw = include_str!("../../hidlins-genpw/src/assets/eff_large_wordlist.txt");
    for word in ["drop-down", "felt-tip", "t-shirt", "yo-yo"] {
        let index = raw
            .lines()
            .position(|line| line.split_once('\t').unwrap().1 == word)
            .unwrap();
        for count in [8, 3] {
            let entropy = u32::try_from(index).unwrap().to_le_bytes().repeat(count);
            let default = PassphraseBuilder::new()
                .words(count)
                .generate_from_bytes(&entropy)
                .unwrap();
            assert_eq!(default.as_str(), vec![word; count].join("-"));
            assert_ne!(
                default.split('-').count(),
                count,
                "hyphen cannot delimit compound-word counts"
            );
            let spaced = PassphraseBuilder::new()
                .words(count)
                .separator(" ")
                .generate_from_bytes(&entropy)
                .unwrap();
            assert_eq!(spaced.as_str(), vec![word; count].join(" "));
            assert_eq!(spaced.split(' ').count(), count);
        }
    }
}

#[test]
fn cli_default_separator_and_explicit_disclosure_remain_compatible() {
    for count in [8, 3] {
        let count_arg = count.to_string();
        let (code, stdout, stderr) = common::run_args(&[
            "--format",
            "json",
            "gen",
            "passphrase",
            "--word-count",
            &count_arg,
            "--show",
        ]);
        assert_eq!(code, 0, "{stderr}");
        let view: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(view["word_count"], count);
        assert_eq!(view["separator"], "-");
        assert_eq!(view["wordlist"], "eff-large");
        assert!(!view["passphrase"].as_str().unwrap().is_empty());
    }
}
