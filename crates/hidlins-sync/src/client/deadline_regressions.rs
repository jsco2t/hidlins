//! Trickle traffic cannot restart the client's absolute I/O deadline.

#[path = "../../tests/common/deadline_fixture.rs"]
mod fixture;

use super::*;
use crate::{framing::PREFACE_LENGTH, protocol::RemoteVersion};

#[test]
fn client_handshake_deadline_survives_partial_frame_progress() {
    let identity = NoiseKeypair::generate().unwrap();
    let server = NoiseKeypair::generate().unwrap();
    let pinned = server.public_key();
    let (stream, peer) = fixture::sockets();
    fixture::check(
        stream,
        peer,
        move |stream, deadline| {
            LanSession::handshake(
                stream,
                &identity,
                pinned,
                deadline,
                LanCancellation::default(),
            )
            .is_err()
        },
        move |peer, stopped, progress| {
            let mut preface = [0_u8; PREFACE_LENGTH];
            peer.read_exact(&mut preface).unwrap();
            assert_eq!(
                Preface::decode(&preface).unwrap().mode(),
                PrefaceMode::Trusted
            );
            let first = fixture::read_frame(peer);
            let mut responder =
                HandshakeSession::trusted_responder_for_authorization(&server).unwrap();
            let mut buffer = [0_u8; 512];
            responder.read_handshake(&first, &mut buffer).unwrap();
            let count = responder.write_handshake(&[], &mut buffer).unwrap();
            let frame = crate::framing::encode_noise_frame(&buffer[..count]).unwrap();
            fixture::send(peer, &frame, true, stopped, progress);
        },
    );
}

fn receive_case(slow: bool) {
    let (client, mut server) = fixture::noise_pair();
    let version = RemoteVersion::new([7; 32]);
    let frame = fixture::record(
        &mut server,
        &Message::HeadResponse {
            version: Some(version),
        },
    );
    let (stream, peer) = fixture::sockets();
    fixture::check(
        stream,
        peer,
        move |stream, deadline| {
            let mut protocol = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
            protocol.authenticate();
            protocol
                .apply(Direction::Send, 1, &Message::HeadRequest)
                .unwrap();
            let mut session = LanSession {
                stream,
                transport: client,
                protocol,
                deadline,
                cancellation: LanCancellation::default(),
            };
            let result = session.receive();
            if slow {
                result.is_err()
            } else {
                matches!(result, Ok((1, Message::HeadResponse { version: value })) if value == Some(version))
            }
        },
        move |peer, stopped, progress| fixture::send(peer, &frame, slow, stopped, progress),
    );
}

#[test]
fn client_authenticated_deadline_survives_partial_frame_progress() {
    receive_case(true);
}

#[test]
fn client_accepts_a_complete_valid_record_before_deadline() {
    receive_case(false);
}
