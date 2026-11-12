//! Synthetic data only. Observers report booleans and never print key material.

use hidlins_erasure_observer as observer;
use snow::{
    params::HashChoice,
    resolvers::{CryptoResolver, DefaultResolver},
    types::Hash,
    Builder,
};
use zeroize::Zeroizing;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/hash_vectors.rs"));

struct WireVector {
    pattern: &'static str,
    prologue: &'static str,
    initiator: &'static str,
    initiator_ephemeral: &'static str,
    remote: &'static str,
    responder: &'static str,
    responder_ephemeral: &'static str,
    handshake_hash: &'static str,
    messages: &'static [(&'static str, &'static str)],
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/wire_vectors.rs"));

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn hash(choice: HashChoice) -> Box<dyn Hash> {
    DefaultResolver::default().resolve_hash(&choice).unwrap()
}

fn storage_observed() {
    assert!(
        observer::count("stored-buffer") > 0,
        "old storage must be erased before the hash wrapper is dropped"
    );
}

fn require_storage() {
    if cfg!(feature = "legacy") {
        observer::assert_erased(&["stored-buffer"]);
    } else {
        observer::assert_erased(&["stored-buffer", "sha-core"]);
    }
}

fn require_result() {
    if cfg!(feature = "legacy") {
        observer::assert_erased(&["stored-buffer"]);
    } else {
        observer::assert_erased(&[
            "stored-buffer",
            "sha-core",
            "hash-output",
            "digest-output",
            "padding",
        ]);
    }
}

#[test]
fn abandoned_hash_and_explicit_reset_erase_owned_storage() {
    for choice in [HashChoice::SHA256, HashChoice::SHA512] {
        observer::reset();
        let mut abandoned = hash(choice);
        abandoned.input(b"recognizable synthetic secret partial block");
        drop(abandoned);
        require_storage();

        observer::reset();
        let mut reset = hash(choice);
        reset.input(b"another synthetic partial block");
        reset.reset();
        storage_observed();
        require_storage();
        drop(reset);
    }
}

#[test]
fn padding_boundaries_result_and_reuse_keep_digest_values_and_erase_storage() {
    for (length, sha256, sha512) in HASH_VECTORS {
        for (choice, expected) in [(HashChoice::SHA256, sha256), (HashChoice::SHA512, sha512)] {
            observer::reset();
            let mut hasher = hash(choice);
            let size = hasher.hash_len();
            let input = vec![0xa5; *length];
            let mut output = Zeroizing::new([0_u8; 64]);
            for _ in 0..2 {
                hasher.input(&input);
                hasher.result(&mut output[..size]);
                assert!(
                    hex(&output[..size]) == *expected,
                    "digest known answer or reuse changed"
                );
                storage_observed();
                require_result();
            }
            drop(hasher);
            require_result();
        }
    }
}

#[test]
fn hmac_known_answers_keep_values_and_erase_owned_scratch() {
    // Existing Snow test vectors; both selected RustCrypto SHA implementations.
    for (choice, expected) in [
        (HashChoice::SHA256, "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
        (HashChoice::SHA512, "fa73b0089d56a284efb0f0756c890be9b1b5dbdd8ee81a3655f83e33b2279d39bf3e848279a722c806b485a47e67c807b946a337bee8942674278859e13292fb"),
    ] {
        observer::reset();
        let mut hasher = hash(choice);
        let mut output = Zeroizing::new([0_u8; 64]);
        let size = hasher.hash_len();
        hasher.hmac(&[0xaa; 20], &[0xdd; 50], &mut output[..size]);
        assert!(hex(&output[..size]) == expected, "HMAC known answer changed");
        require_result();
        drop(hasher);
        require_result();
    }
}

#[test]
fn hkdf_all_output_counts_erase_owned_scratch() {
    // RFC 5869 A.3 factual known-answer prefix, empty salt/info. No RFC code.
    // https://www.rfc-editor.org/rfc/rfc5869#appendix-A.3
    let known = unhex(
        "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8",
    );
    for choice in [HashChoice::SHA256, HashChoice::SHA512] {
        for outputs in 1..=3 {
            observer::reset();
            let mut hasher = hash(choice);
            let mut a = Zeroizing::new([0_u8; 64]);
            let mut b = Zeroizing::new([0_u8; 64]);
            let mut c = Zeroizing::new([0_u8; 64]);
            hasher.hkdf(&[], &[0x0b; 22], outputs, &mut *a, &mut *b, &mut *c);
            if choice == HashChoice::SHA256 {
                assert!(a[..32] == known[..32], "HKDF first output changed");
                if outputs > 1 {
                    assert!(b[..10] == known[32..], "HKDF second-output prefix changed");
                }
            }
            require_result();
            drop(hasher);
            require_result();
        }
    }
}

#[test]
fn fixed_noise_wire_vectors_preserve_handshake_and_transport_bytes() {
    for vector in WIRE_VECTORS {
        observer::reset();
        let a_key = unhex(vector.initiator);
        let b_key = unhex(vector.responder);
        let a_eph = unhex(vector.initiator_ephemeral);
        let b_eph = unhex(vector.responder_ephemeral);
        let prologue = unhex(vector.prologue);
        let remote = unhex(vector.remote);
        let mut a_builder = Builder::new(vector.pattern.parse().unwrap())
            .local_private_key(&a_key)
            .unwrap()
            .prologue(&prologue)
            .unwrap()
            .fixed_ephemeral_key_for_testing_only(&a_eph);
        if !remote.is_empty() {
            a_builder = a_builder.remote_public_key(&remote).unwrap();
        }
        let mut a = a_builder.build_initiator().unwrap();
        let mut b = Builder::new(vector.pattern.parse().unwrap())
            .local_private_key(&b_key)
            .unwrap()
            .prologue(&prologue)
            .unwrap()
            .fixed_ephemeral_key_for_testing_only(&b_eph)
            .build_responder()
            .unwrap();
        let handshake_messages = if vector.pattern.contains("_XX_") {
            3
        } else {
            2
        };
        let mut wire = [0_u8; 2048];
        let mut plain = [0_u8; 2048];
        for (index, (payload, ciphertext)) in
            vector.messages[..handshake_messages].iter().enumerate()
        {
            let payload = unhex(payload);
            let (sender, receiver) = if index % 2 == 0 {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            let length = sender.write_message(&payload, &mut wire).unwrap();
            assert!(
                hex(&wire[..length]) == *ciphertext,
                "Noise handshake known answer changed"
            );
            let received = receiver.read_message(&wire[..length], &mut plain).unwrap();
            assert!(
                plain[..received] == payload,
                "Noise handshake payload changed"
            );
        }
        assert!(hex(a.get_handshake_hash()) == vector.handshake_hash);
        assert!(hex(b.get_handshake_hash()) == vector.handshake_hash);
        let mut a = a.into_transport_mode().unwrap();
        let mut b = b.into_transport_mode().unwrap();
        for (index, (payload, ciphertext)) in
            vector.messages[handshake_messages..].iter().enumerate()
        {
            let payload = unhex(payload);
            // Vector messages continue alternating from the handshake.
            let (sender, receiver) = if (index + handshake_messages) % 2 == 0 {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            let length = sender.write_message(&payload, &mut wire).unwrap();
            assert!(
                hex(&wire[..length]) == *ciphertext,
                "Noise transport known answer changed"
            );
            let received = receiver.read_message(&wire[..length], &mut plain).unwrap();
            assert!(plain[..received] == payload);
        }
        drop(a);
        drop(b);
        require_result();
    }
}

#[test]
fn abandoned_noise_handshakes_erase_hash_storage() {
    for vector in WIRE_VECTORS {
        observer::reset();
        let key = unhex(vector.initiator);
        let remote = unhex(vector.remote);
        let mut builder = Builder::new(vector.pattern.parse().unwrap())
            .local_private_key(&key)
            .unwrap();
        if !remote.is_empty() {
            builder = builder.remote_public_key(&remote).unwrap();
        }
        let mut handshake = builder.build_initiator().unwrap();
        let mut message = [0_u8; 512];
        handshake
            .write_message(b"synthetic handshake payload", &mut message)
            .unwrap();
        drop(handshake);
        require_storage();
    }
}

#[test]
#[cfg(panic = "unwind")]
fn unwind_and_short_output_erase_owned_storage_and_scratch() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for choice in [HashChoice::SHA256, HashChoice::SHA512] {
        observer::reset();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let mut hasher = hash(choice);
            hasher.input(b"synthetic partial block before unwind");
            panic!("intentional fixture boundary");
        }))
        .is_err());
        require_storage();

        observer::reset();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let mut hasher = hash(choice);
            hasher.input(b"synthetic input before output bounds failure");
            hasher.result(&mut [0_u8; 1]);
        }))
        .is_err());
        require_result();
    }
}
