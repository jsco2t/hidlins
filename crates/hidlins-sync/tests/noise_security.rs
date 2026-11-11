//! LNS-NOISE conformance tests for the narrow Hidlins Noise wrapper.

use hidlins_sync::noise::{
    HandshakeError, HandshakeSession, NoiseKeypair, SessionMode, HANDSHAKE_TIMEOUT,
    PAIRING_PATTERN, PREFACE_PAIRING, PREFACE_TRUSTED, TRUSTED_PATTERN,
};

fn complete_xx(
    initiator: &mut HandshakeSession,
    responder: &mut HandshakeSession,
) -> Result<(), HandshakeError> {
    let mut wire = vec![0_u8; 65_535];
    let mut plain = vec![0_u8; 65_535];

    let n = initiator.write_handshake(b"", &mut wire)?;
    responder.read_handshake(&wire[..n], &mut plain)?;
    let n = responder.write_handshake(b"", &mut wire)?;
    initiator.read_handshake(&wire[..n], &mut plain)?;
    let n = initiator.write_handshake(b"", &mut wire)?;
    responder.read_handshake(&wire[..n], &mut plain)?;
    Ok(())
}

fn complete_ik(
    initiator: &mut HandshakeSession,
    responder: &mut HandshakeSession,
) -> Result<(), HandshakeError> {
    let mut wire = vec![0_u8; 65_535];
    let mut plain = vec![0_u8; 65_535];

    let n = initiator.write_handshake(b"", &mut wire)?;
    responder.read_handshake(&wire[..n], &mut plain)?;
    let n = responder.write_handshake(b"", &mut wire)?;
    initiator.read_handshake(&wire[..n], &mut plain)?;
    Ok(())
}

// LNS-NOISE-001
#[test]
fn fixed_protocol_constants_are_exact() {
    assert_eq!(PAIRING_PATTERN, "Noise_XX_25519_ChaChaPoly_SHA256");
    assert_eq!(TRUSTED_PATTERN, "Noise_IK_25519_ChaChaPoly_SHA256");
    assert_eq!(PREFACE_PAIRING, b"HIDLINS\0\x01\x01\0\0");
    assert_eq!(PREFACE_TRUSTED, b"HIDLINS\0\x01\x02\0\0");
    assert_eq!(HANDSHAKE_TIMEOUT.as_secs(), 5);
}

// LNS-NOISE-002
#[test]
fn xx_authenticates_both_static_keys_and_enters_transport_mode() {
    let client = NoiseKeypair::generate().expect("client keypair");
    let server = NoiseKeypair::generate().expect("server keypair");
    let mut initiator = HandshakeSession::pairing_initiator(&client).expect("initiator");
    let mut responder = HandshakeSession::pairing_responder(&server).expect("responder");

    complete_xx(&mut initiator, &mut responder).expect("XX handshake");
    let mut client_transport = initiator.finish().expect("client transport");
    let mut server_transport = responder.finish().expect("server transport");

    assert_eq!(client_transport.mode(), SessionMode::Pairing);
    assert_eq!(server_transport.mode(), SessionMode::Pairing);
    assert_eq!(client_transport.peer_static(), server.public_key());
    assert_eq!(server_transport.peer_static(), client.public_key());
    assert_eq!(
        client_transport.handshake_hash(),
        server_transport.handshake_hash()
    );

    let mut ciphertext = [0_u8; 128];
    let mut plaintext = [0_u8; 128];
    let n = client_transport
        .write_message(b"encrypted application record", &mut ciphertext)
        .expect("encrypt");
    let m = server_transport
        .read_message(&ciphertext[..n], &mut plaintext)
        .expect("decrypt");
    assert_eq!(&plaintext[..m], b"encrypted application record");
}

// LNS-NOISE-003
#[test]
fn ik_requires_the_pinned_static_keys_on_both_ends() {
    let client = NoiseKeypair::generate().expect("client keypair");
    let server = NoiseKeypair::generate().expect("server keypair");
    let mut initiator =
        HandshakeSession::trusted_initiator(&client, server.public_key()).expect("initiator");
    let mut responder =
        HandshakeSession::trusted_responder(&server, client.public_key()).expect("responder");

    complete_ik(&mut initiator, &mut responder).expect("IK handshake");
    let client_transport = initiator.finish().expect("client transport");
    let server_transport = responder.finish().expect("server transport");
    assert_eq!(client_transport.mode(), SessionMode::Trusted);
    assert_eq!(client_transport.peer_static(), server.public_key());
    assert_eq!(server_transport.peer_static(), client.public_key());
}

// LNS-NOISE-004
#[test]
fn ik_wrong_pinned_server_key_fails_closed() {
    let client = NoiseKeypair::generate().expect("client keypair");
    let server = NoiseKeypair::generate().expect("server keypair");
    let impostor = NoiseKeypair::generate().expect("impostor keypair");
    let mut initiator =
        HandshakeSession::trusted_initiator(&client, impostor.public_key()).expect("initiator");
    let mut responder =
        HandshakeSession::trusted_responder(&server, client.public_key()).expect("responder");

    let error = complete_ik(&mut initiator, &mut responder).expect_err("pin mismatch must fail");
    assert!(matches!(error, HandshakeError::CryptographicFailure));
}

// LNS-NOISE-005
#[test]
fn pairing_and_trusted_modes_cannot_be_crossed() {
    let client = NoiseKeypair::generate().expect("client keypair");
    let server = NoiseKeypair::generate().expect("server keypair");
    let mut pairing = HandshakeSession::pairing_initiator(&client).expect("pairing");
    let mut trusted =
        HandshakeSession::trusted_responder(&server, client.public_key()).expect("trusted");

    let mut wire = vec![0_u8; 65_535];
    let mut plain = vec![0_u8; 65_535];
    let n = pairing
        .write_handshake(b"", &mut wire)
        .expect("XX first message");
    let error = trusted
        .read_handshake(&wire[..n], &mut plain)
        .expect_err("different pattern/prologue must fail");
    assert!(matches!(error, HandshakeError::CryptographicFailure));
}

// LNS-NOISE-006
#[test]
fn secret_bearing_debug_output_is_redacted() {
    let keypair = NoiseKeypair::generate().expect("keypair");
    let debug = format!("{keypair:?}");
    assert_eq!(debug, "NoiseKeypair([REDACTED])");

    let session = HandshakeSession::pairing_initiator(&keypair).expect("session");
    let debug = format!("{session:?}");
    assert!(!debug.contains(&format!("{:?}", keypair.public_key())));
    assert!(debug.contains("[REDACTED]"));
}
