use hidlins_genpw::{CharSet, PassphraseBuilder, PasswordBuilder};

use super::session::AppSession;
use crate::dto::{GeneratedSecret, PassphraseOptionsDto, PasswordOptionsDto};
use crate::error::HidlinsApiError;

const DICEWARE_WORDLIST_SIZE: f64 = 7776.0;

impl AppSession {
    #[allow(clippy::needless_pass_by_value)]
    pub fn generate_password(
        &self,
        opts: PasswordOptionsDto,
    ) -> Result<GeneratedSecret, HidlinsApiError> {
        let classes = CharSet {
            lowercase: opts.lowercase,
            uppercase: opts.uppercase,
            digits: opts.digits,
            symbols: opts.symbols,
        };

        let value = PasswordBuilder::new()
            .length(opts.length)
            .classes(classes)
            .exclude_ambiguous(opts.exclude_ambiguous)
            .generate()?;

        let entropy_bits = compute_password_entropy(opts.length, classes, opts.exclude_ambiguous);

        Ok(GeneratedSecret {
            value: (*value).clone(),
            entropy_bits,
        })
    }

    #[allow(clippy::needless_pass_by_value, clippy::cast_precision_loss)]
    pub fn generate_passphrase(
        &self,
        opts: PassphraseOptionsDto,
    ) -> Result<GeneratedSecret, HidlinsApiError> {
        let value = PassphraseBuilder::new()
            .words(opts.words)
            .separator(opts.separator)
            .generate()?;

        let entropy_bits = opts.words as f64 * DICEWARE_WORDLIST_SIZE.log2();

        Ok(GeneratedSecret {
            value: (*value).clone(),
            entropy_bits,
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn compute_password_entropy(length: usize, classes: CharSet, exclude_ambiguous: bool) -> f64 {
    const LOWERCASE_LEN: usize = 26;
    const UPPERCASE_LEN: usize = 26;
    const DIGITS_LEN: usize = 10;
    const SYMBOLS_LEN: usize = 27;
    const AMBIGUOUS: &str = "0Oo1lI|`";

    let mut alphabet_size: usize = 0;

    if classes.lowercase {
        let base = LOWERCASE_LEN;
        let filtered = if exclude_ambiguous {
            base - AMBIGUOUS.chars().filter(char::is_ascii_lowercase).count()
        } else {
            base
        };
        alphabet_size += filtered;
    }
    if classes.uppercase {
        let base = UPPERCASE_LEN;
        let filtered = if exclude_ambiguous {
            base - AMBIGUOUS.chars().filter(char::is_ascii_uppercase).count()
        } else {
            base
        };
        alphabet_size += filtered;
    }
    if classes.digits {
        let base = DIGITS_LEN;
        let filtered = if exclude_ambiguous {
            base - AMBIGUOUS.chars().filter(char::is_ascii_digit).count()
        } else {
            base
        };
        alphabet_size += filtered;
    }
    if classes.symbols {
        let base = SYMBOLS_LEN;
        let filtered = if exclude_ambiguous {
            base - AMBIGUOUS
                .chars()
                .filter(|c| {
                    !c.is_ascii_alphanumeric() && "!@#$%^&*()-_=+[]{};:,.<>?/~".contains(*c)
                })
                .count()
        } else {
            base
        };
        alphabet_size += filtered;
    }

    if alphabet_size == 0 {
        return 0.0;
    }

    length as f64 * (alphabet_size as f64).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_all_classes_no_filter() {
        let bits = compute_password_entropy(20, CharSet::ALL, false);
        let expected = 20.0 * (89_f64).log2();
        assert!((bits - expected).abs() < 0.01);
    }

    #[test]
    fn entropy_all_classes_with_filter() {
        let bits = compute_password_entropy(20, CharSet::ALL, true);
        // 26 lowercase - 1 ('l') = 25
        // 26 uppercase - 2 ('O', 'I') = 24
        // 10 digits - 2 ('0', '1') = 8
        // 27 symbols - 1 ('|') = 26
        // Total: 25 + 24 + 8 + 26 = 83
        let expected = 20.0 * (83_f64).log2();
        assert!((bits - expected).abs() < 0.01);
    }

    #[test]
    fn entropy_diceware() {
        let bits = 6.0 * DICEWARE_WORDLIST_SIZE.log2();
        let expected = 6.0 * 7776_f64.log2();
        assert!((bits - expected).abs() < 0.01);
        assert!(bits > 77.0 && bits < 78.0);
    }

    #[test]
    fn entropy_digits_only() {
        let bits = compute_password_entropy(
            4,
            CharSet {
                lowercase: false,
                uppercase: false,
                digits: true,
                symbols: false,
            },
            false,
        );
        let expected = 4.0 * (10_f64).log2();
        assert!((bits - expected).abs() < 0.01);
    }

    #[test]
    fn entropy_lowercase_only() {
        let bits = compute_password_entropy(
            12,
            CharSet {
                lowercase: true,
                uppercase: false,
                digits: false,
                symbols: false,
            },
            false,
        );
        let expected = 12.0 * (26_f64).log2();
        assert!((bits - expected).abs() < 0.01);
    }
}
