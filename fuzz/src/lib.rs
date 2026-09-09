//! Stable harness logic shared by libFuzzer and committed-corpus replay.

#![forbid(unsafe_code)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use hidlins_sync::{
    address::LocalEndpoint,
    framing::{decode_noise_frame, Preface},
    protocol::{Direction, LocalRole, Message, ProtocolState, SessionMode},
};

/// Exercise strict record and frame decoding, including canonical re-encoding.
pub fn protocol_input(input: &[u8]) {
    if let Ok(decoded) = Message::decode(input) {
        let encoded = decoded
            .message
            .encode(decoded.request_id)
            .expect("a decoded message must remain encodable");
        assert_eq!(
            encoded, input,
            "accepted protocol records must be canonical"
        );
    }
    let _ = Preface::decode(input);
    let _ = decode_noise_frame(input);
}

/// Exercise every address constructor and canonical literal round trip.
pub fn address_input(input: &[u8]) {
    if let Ok(text) = std::str::from_utf8(input) {
        if let Ok(endpoint) = text.parse::<LocalEndpoint>() {
            let reparsed = endpoint
                .to_string()
                .parse::<LocalEndpoint>()
                .expect("a displayed checked endpoint must parse");
            assert_eq!(endpoint, reparsed);
        }
    }

    if input.len() >= 7 {
        let port = u16::from_be_bytes([input[0], input[1]]);
        let scope = u32::from_be_bytes([input[2], input[3], input[4], input[5]]);
        let _ = LocalEndpoint::new(
            IpAddr::V4(Ipv4Addr::new(input[3], input[4], input[5], input[6])),
            port,
            scope,
        );
    }
    if input.len() >= 22 {
        let mut octets = [0_u8; 16];
        octets.copy_from_slice(&input[6..22]);
        let _ = LocalEndpoint::new(
            IpAddr::V6(Ipv6Addr::from(octets)),
            port(input),
            scope(input),
        );
    }
}

/// Exercise message-order, replay, duplicate, and cross-mode state transitions.
pub fn state_input(input: &[u8]) {
    let mode = if input.first().is_some_and(|byte| byte & 1 == 0) {
        SessionMode::Trusted
    } else {
        SessionMode::Pairing
    };
    let role = if input.first().is_some_and(|byte| byte & 2 == 0) {
        LocalRole::Client
    } else {
        LocalRole::Server
    };
    let mut state = ProtocolState::new(mode, role);
    if input.first().is_some_and(|byte| byte & 4 != 0) {
        state.authenticate();
    }

    let mut cursor = 1_usize;
    while cursor + 3 <= input.len() {
        let direction = if input[cursor] & 1 == 0 {
            Direction::Send
        } else {
            Direction::Receive
        };
        let length = usize::from(u16::from_be_bytes([input[cursor + 1], input[cursor + 2]]));
        cursor += 3;
        let Some(end) = cursor.checked_add(length).filter(|end| *end <= input.len()) else {
            break;
        };
        if let Ok(decoded) = Message::decode(&input[cursor..end]) {
            let _ = state.apply(direction, decoded.request_id, &decoded.message);
        }
        cursor = end;
    }
}

fn port(input: &[u8]) -> u16 {
    u16::from_be_bytes([input[0], input[1]])
}

fn scope(input: &[u8]) -> u32 {
    u32::from_be_bytes([input[2], input[3], input[4], input[5]])
}
