use super::*;
use crate::entropy_allocator::DeallocationWatch;

#[test]
fn injected_draws_keep_exact_output() {
    let result = PasswordBuilder::new()
        .generate_with_entropy(|buffer| {
            buffer.fill(0);
            Ok(())
        })
        .unwrap();
    assert_eq!(result.as_str(), format!("A0!{}", "a".repeat(17)));
}

#[test]
fn partial_fill_failure_remains_csprng_error() {
    let mut watch = None;
    let result = PasswordBuilder::new().generate_with_entropy(|buffer| {
        buffer[..7].fill(0x5a);
        watch = Some(DeallocationWatch::new(buffer));
        Err(getrandom::Error::UNEXPECTED)
    });
    assert!(matches!(result, Err(GenError::Csprng(_))));
    watch.unwrap().assert_erased();
}

#[test]
fn successful_generation_erases_entropy() {
    let mut watch = None;
    let result = PasswordBuilder::new().generate_with_entropy(|buffer| {
        buffer.fill(0x5a);
        watch = Some(DeallocationWatch::new(buffer));
        Ok(())
    });
    assert!(result.is_ok());
    watch.unwrap().assert_erased();
}

#[test]
fn provider_unwind_erases_initialized_entropy() {
    let mut watch = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PasswordBuilder::new().generate_with_entropy(|buffer| {
            buffer[..7].fill(0x5a);
            watch = Some(DeallocationWatch::new(buffer));
            panic!("injected entropy provider panic");
        })
    }));
    assert!(result.is_err());
    watch.unwrap().assert_erased();
}
