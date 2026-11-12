//! Address-policy, framing, codec, and protocol-state conformance tests.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use hidlins_sync::{
    address::{DiscoveryDestination, LocalBindEndpoint, LocalEndpoint},
    framing::{decode_noise_frame, encode_noise_frame, Preface, PrefaceMode},
    protocol::{
        Direction, ErrorCode, LocalRole, Message, ProtocolError, ProtocolState, RemoteVersion,
        ResourceLimit, SessionMode, AUTHENTICATED_IDLE_TIMEOUT, CANDIDATE_CONNECT_TIMEOUT,
        MAX_ACTIVE_SAS_CANDIDATES, MAX_APPLICATION_CHUNK, MAX_CANDIDATE_ATTEMPTS,
        MAX_CONDITIONAL_COMMIT_RETRIES, MAX_CONNECTIONS_PER_VAULT, MAX_DISCOVERY_ENDPOINTS,
        MAX_ENCRYPTED_VAULT, MAX_PAIRING_FAILURES, MAX_PENDING_HOST_OPERATIONS, PAIRING_WINDOW,
        SYNC_SESSION_TIMEOUT, TOTAL_CONNECT_BUDGET,
    },
};

fn version(byte: u8) -> RemoteVersion {
    RemoteVersion::new([byte; 32])
}

// LNS-ADDR-001
#[test]
fn address_ipv4_policy_matches_the_complete_first_two_octet_table() {
    for first in 0_u8..=u8::MAX {
        for second in 0_u8..=u8::MAX {
            let ip = Ipv4Addr::new(first, second, 23, 42);
            let expected = first == 10
                || (first == 172 && (16..=31).contains(&second))
                || (first == 192 && second == 168)
                || (first == 169 && second == 254)
                || first == 127;
            assert_eq!(
                LocalEndpoint::new(IpAddr::V4(ip), 4242, 0).is_ok(),
                expected,
                "classification mismatch for {ip}"
            );
        }
    }
}

#[test]
fn address_ephemeral_port_exists_only_in_the_checked_bind_type() {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert!(LocalEndpoint::new(ip, 0, 0).is_err());
    let bind = LocalBindEndpoint::ephemeral(ip, 0).expect("allowed bind interface");
    assert_eq!(bind.socket_addr().port(), 0);
    assert!(LocalBindEndpoint::ephemeral("8.8.8.8".parse().unwrap(), 0).is_err());
    assert!(LocalBindEndpoint::ephemeral("224.0.0.251".parse().unwrap(), 0).is_err());
}

// LNS-ADDR-001b
#[test]
fn address_ipv4_prefix_edges_and_named_special_ranges_are_exact() {
    for allowed in [
        "10.0.0.0",
        "10.255.255.255",
        "172.16.0.0",
        "172.31.255.255",
        "192.168.0.0",
        "192.168.255.255",
        "169.254.0.0",
        "169.254.255.255",
        "127.0.0.0",
        "127.255.255.255",
    ] {
        assert!(
            LocalEndpoint::new(allowed.parse().unwrap(), 1, 0).is_ok(),
            "{allowed}"
        );
    }
    for rejected in [
        "9.255.255.255",
        "11.0.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "192.167.255.255",
        "192.169.0.0",
        "169.253.255.255",
        "169.255.0.0",
        "100.64.0.0",
        "100.127.255.255",
        "192.0.2.1",
        "198.18.0.1",
        "224.0.0.1",
        "0.0.0.0",
        "255.255.255.255",
        "8.8.8.8",
    ] {
        assert!(
            LocalEndpoint::new(rejected.parse().unwrap(), 1, 0).is_err(),
            "{rejected}"
        );
    }
}

// LNS-ADDR-002
#[test]
fn address_mapped_ipv6_is_normalized_before_policy_and_cannot_bypass_it() {
    let private = LocalEndpoint::new(
        IpAddr::V6(Ipv4Addr::new(192, 168, 1, 8).to_ipv6_mapped()),
        4242,
        0,
    )
    .expect("mapped private address");
    assert_eq!(private.ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 8)));

    assert!(LocalEndpoint::new(
        IpAddr::V6(Ipv4Addr::new(8, 8, 8, 8).to_ipv6_mapped()),
        4242,
        0,
    )
    .is_err());
}

// LNS-ADDR-003
#[test]
fn address_ipv6_policy_requires_canonical_scope_and_rejects_special_ranges() {
    for allowed in ["fc00::1", "fdff:ffff::1", "::1"] {
        let ip: Ipv6Addr = allowed.parse().unwrap();
        assert!(LocalEndpoint::new(IpAddr::V6(ip), 4242, 0).is_ok(), "{ip}");
    }
    let link_local: Ipv6Addr = "fe80::1".parse().unwrap();
    assert!(LocalEndpoint::new(IpAddr::V6(link_local), 4242, 0).is_err());
    assert_eq!(
        LocalEndpoint::new(IpAddr::V6(link_local), 4242, 7)
            .unwrap()
            .scope_id(),
        7
    );
    assert!(LocalEndpoint::new("fd00::1".parse().unwrap(), 4242, 7).is_err());

    for rejected in [
        "::",
        "::2",
        "fe7f::1",
        "fec0::1",
        "ff02::1",
        "2001:db8::1",
        "2001::1",
    ] {
        assert!(
            LocalEndpoint::new(rejected.parse().unwrap(), 4242, 0).is_err(),
            "{rejected}"
        );
    }
}

// LNS-ADDR-003b
#[test]
fn address_ipv6_first_segment_property_allows_only_ula_and_link_local_prefixes() {
    for first in 0_u16..=u16::MAX {
        let ip = Ipv6Addr::new(first, 0, 0, 0, 0, 0, 0, 1);
        let link_local = first & 0xffc0 == 0xfe80;
        let unique_local = first & 0xfe00 == 0xfc00;
        let loopback = ip.is_loopback();
        let scope = u32::from(link_local);
        assert_eq!(
            LocalEndpoint::new(IpAddr::V6(ip), 4242, scope).is_ok(),
            link_local || unique_local || loopback,
            "classification mismatch for {ip}"
        );
    }
}

// LNS-ADDR-004
#[test]
fn address_literal_parser_has_no_dns_or_ambiguous_scope_fallback() {
    assert_eq!(
        "192.168.4.2:7331"
            .parse::<LocalEndpoint>()
            .unwrap()
            .to_string(),
        "192.168.4.2:7331"
    );
    assert_eq!(
        "[fe80::abcd%9]:7331"
            .parse::<LocalEndpoint>()
            .unwrap()
            .to_string(),
        "[fe80::abcd%9]:7331"
    );
    for bad in [
        "host.local:7331",
        "192.168.1.2",
        "192.168.1.2:0",
        "[fe80::1]:7331",
        "[fe80::1%0]:7331",
        "[fd00::1%2]:7331",
        "[fd00::1]:0",
        "127.0.0.1:7331 trailing",
    ] {
        assert!(bad.parse::<LocalEndpoint>().is_err(), "{bad}");
    }
}

// LNS-ADDR-005
#[test]
fn address_discovery_multicast_type_cannot_become_a_sync_endpoint() {
    assert!(LocalEndpoint::new("224.0.0.251".parse().unwrap(), 5353, 0).is_err());
    assert_eq!(
        DiscoveryDestination::mdns_v4().socket_addr().to_string(),
        "224.0.0.251:5353"
    );
    assert!(DiscoveryDestination::mdns_v6(0).is_err());
    assert!(DiscoveryDestination::mdns_v6(4).is_ok());
}

// LNS-FRAME-001
#[test]
fn protocol_preface_and_noise_frames_are_exact_and_bounded() {
    for mode in [PrefaceMode::Pairing, PrefaceMode::Trusted] {
        let bytes = Preface::new(mode).encode();
        assert_eq!(bytes.len(), 12);
        assert_eq!(Preface::decode(&bytes).unwrap().mode(), mode);
    }
    for mutation in [0_usize, 8, 9, 10, 11] {
        let mut bytes = Preface::new(PrefaceMode::Pairing).encode();
        bytes[mutation] ^= 0xff;
        assert!(Preface::decode(&bytes).is_err());
    }
    assert!(Preface::decode(&Preface::new(PrefaceMode::Pairing).encode()[..11]).is_err());

    for size in [1_usize, 65_534, 65_535] {
        let payload = vec![0x5a; size];
        let frame = encode_noise_frame(&payload).unwrap();
        assert_eq!(decode_noise_frame(&frame).unwrap(), payload);
    }
    assert!(encode_noise_frame(&[]).is_err());
    assert!(encode_noise_frame(&vec![0; 65_536]).is_err());
    assert!(decode_noise_frame(&[0, 0]).is_err());
    assert!(decode_noise_frame(&[0, 2, 1]).is_err());
    assert!(decode_noise_frame(&[0, 1, 1, 2]).is_err());
}

fn messages() -> Vec<(u32, Message)> {
    vec![
        (1, Message::HeadRequest),
        (1, Message::HeadResponse { version: None }),
        (
            1,
            Message::HeadResponse {
                version: Some(version(1)),
            },
        ),
        (2, Message::FetchRequest { known: None }),
        (
            2,
            Message::FetchRequest {
                known: Some(version(2)),
            },
        ),
        (2, Message::FetchUnchanged),
        (
            2,
            Message::FetchBegin {
                version: version(3),
                total_length: 10,
                digest: [4; 32],
            },
        ),
        (
            2,
            Message::FetchChunk {
                sequence: 0,
                bytes: vec![1, 2, 3],
            },
        ),
        (2, Message::FetchCommit { chunk_count: 1 }),
        (
            3,
            Message::UploadBegin {
                expected: None,
                total_length: 0,
                digest: [5; 32],
            },
        ),
        (
            3,
            Message::UploadBegin {
                expected: Some(version(6)),
                total_length: 3,
                digest: [7; 32],
            },
        ),
        (
            3,
            Message::UploadChunk {
                sequence: 0,
                bytes: vec![8, 9],
            },
        ),
        (3, Message::UploadCommit { chunk_count: 1 }),
        (
            3,
            Message::UploadAccepted {
                version: version(8),
            },
        ),
        (3, Message::Cancel),
        (
            3,
            Message::Error {
                code: ErrorCode::Busy,
            },
        ),
        (
            4,
            Message::PairCommit {
                transaction_id: [9; 16],
                transcript_digest: [10; 32],
            },
        ),
        (
            4,
            Message::PairPrepared {
                transaction_id: [9; 16],
                transcript_digest: [10; 32],
            },
        ),
        (
            4,
            Message::PairActivate {
                transaction_id: [9; 16],
                transcript_digest: [10; 32],
            },
        ),
        (
            4,
            Message::PairActivated {
                transaction_id: [9; 16],
                transcript_digest: [10; 32],
            },
        ),
    ]
}

// LNS-PROTO-001
#[test]
fn protocol_all_canonical_messages_round_trip_and_reject_trailing_bytes() {
    for (request_id, message) in messages() {
        let encoded = message.encode(request_id).unwrap();
        let decoded = Message::decode(&encoded).unwrap();
        assert_eq!(decoded.request_id, request_id);
        assert_eq!(decoded.message, message);

        let mut trailing = encoded;
        trailing.push(0);
        assert!(Message::decode(&trailing).is_err());
    }
}

// LNS-PROTO-002
#[test]
fn protocol_lengths_and_allocation_claims_fail_at_boundaries() {
    let equal_chunk = Message::UploadChunk {
        sequence: 0,
        bytes: vec![0; MAX_APPLICATION_CHUNK],
    };
    assert!(equal_chunk.encode(1).is_ok());
    let above_chunk = Message::UploadChunk {
        sequence: 0,
        bytes: vec![0; MAX_APPLICATION_CHUNK + 1],
    };
    assert!(above_chunk.encode(1).is_err());

    for total in [MAX_ENCRYPTED_VAULT - 1, MAX_ENCRYPTED_VAULT] {
        assert!(Message::UploadBegin {
            expected: None,
            total_length: total,
            digest: [0; 32]
        }
        .encode(1)
        .is_ok());
    }
    assert!(Message::UploadBegin {
        expected: None,
        total_length: MAX_ENCRYPTED_VAULT + 1,
        digest: [0; 32]
    }
    .encode(1)
    .is_err());

    let mut claimed = vec![0x06, 0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff];
    claimed.extend_from_slice(&[0; 4]);
    assert!(Message::decode(&claimed).is_err());
}

// LNS-PROTO-003
#[test]
fn protocol_malformed_byte_seeded_inputs_never_panic_or_decode_noncanonically() {
    let mut seed = 0x9e37_79b9_u32;
    for length in 0..512_usize {
        let mut bytes = vec![0_u8; length];
        for byte in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed.to_le_bytes()[0];
        }
        if let Ok(decoded) = Message::decode(&bytes) {
            let canonical = decoded.message.encode(decoded.request_id).unwrap();
            assert_eq!(canonical, bytes);
        }
    }
}

// LNS-PROTO-003b
#[test]
fn protocol_committed_malformed_corpus_stays_rejected() {
    for line in include_str!("corpus/protocol_malformed.hex").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bytes = line
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let text = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(text, 16).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(
            Message::decode(&bytes).is_err(),
            "accepted corpus seed {line}"
        );
    }
}

// LNS-LIMIT-001
#[test]
fn protocol_every_fixed_resource_limit_is_frozen_and_has_boundary_values() {
    assert_eq!(PAIRING_WINDOW.as_secs(), 180);
    assert_eq!(CANDIDATE_CONNECT_TIMEOUT.as_secs(), 2);
    assert_eq!(TOTAL_CONNECT_BUDGET.as_secs(), 10);
    assert_eq!(AUTHENTICATED_IDLE_TIMEOUT.as_secs(), 15);
    assert_eq!(SYNC_SESSION_TIMEOUT.as_secs(), 300);
    for (resource, expected) in [
        (ResourceLimit::EncryptedVault, MAX_ENCRYPTED_VAULT),
        (
            ResourceLimit::ApplicationChunk,
            MAX_APPLICATION_CHUNK as u64,
        ),
        (ResourceLimit::NoiseFrame, 65_535),
        (ResourceLimit::PairingFailures, MAX_PAIRING_FAILURES as u64),
        (
            ResourceLimit::ActiveSasCandidates,
            MAX_ACTIVE_SAS_CANDIDATES as u64,
        ),
        (
            ResourceLimit::CandidateAttempts,
            MAX_CANDIDATE_ATTEMPTS as u64,
        ),
        (
            ResourceLimit::ConnectionsPerVault,
            MAX_CONNECTIONS_PER_VAULT as u64,
        ),
        (
            ResourceLimit::PendingHostOperations,
            MAX_PENDING_HOST_OPERATIONS as u64,
        ),
        (
            ResourceLimit::ConditionalCommitRetries,
            MAX_CONDITIONAL_COMMIT_RETRIES as u64,
        ),
        (
            ResourceLimit::DiscoveryEndpoints,
            MAX_DISCOVERY_ENDPOINTS as u64,
        ),
    ] {
        assert_eq!(resource.maximum(), expected);
        assert!(resource.allows(expected.saturating_sub(1)));
        assert!(resource.allows(expected));
        assert!(!resource.allows(expected.checked_add(1).unwrap()));
    }
    assert_eq!(MAX_ENCRYPTED_VAULT, 268_435_456);
}

// LNS-STATE-001
#[test]
fn protocol_preauthentication_and_pairing_cannot_issue_vault_operations() {
    let mut trusted = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
    assert_eq!(
        trusted.apply(Direction::Send, 1, &Message::HeadRequest),
        Err(ProtocolError::NotAuthorized)
    );
    trusted.authenticate();
    assert!(trusted
        .apply(Direction::Send, 1, &Message::HeadRequest)
        .is_ok());

    let mut pairing = ProtocolState::new(SessionMode::Pairing, LocalRole::Client);
    pairing.authenticate();
    assert_eq!(
        pairing.apply(Direction::Send, 1, &Message::HeadRequest),
        Err(ProtocolError::InvalidState)
    );
}

// LNS-STATE-001b
#[test]
fn protocol_outstanding_requests_are_bounded_without_unbounded_replay_storage() {
    let mut client = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
    client.authenticate();
    for request_id in 1..=u32::try_from(MAX_PENDING_HOST_OPERATIONS).unwrap() {
        client
            .apply(Direction::Send, request_id, &Message::HeadRequest)
            .unwrap();
    }
    let next = u32::try_from(MAX_PENDING_HOST_OPERATIONS + 1).unwrap();
    assert_eq!(
        client.apply(Direction::Send, next, &Message::HeadRequest),
        Err(ProtocolError::InvalidState)
    );
}

// LNS-STATE-002
#[test]
fn protocol_fetch_upload_order_duplicates_and_sequences_are_strict() {
    let mut client = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
    client.authenticate();
    client
        .apply(Direction::Send, 7, &Message::FetchRequest { known: None })
        .unwrap();
    client
        .apply(
            Direction::Receive,
            7,
            &Message::FetchBegin {
                version: version(1),
                total_length: 2,
                digest: [2; 32],
            },
        )
        .unwrap();
    assert_eq!(
        client.apply(
            Direction::Receive,
            7,
            &Message::FetchChunk {
                sequence: 1,
                bytes: vec![1]
            }
        ),
        Err(ProtocolError::InvalidState)
    );
    client
        .apply(
            Direction::Receive,
            7,
            &Message::FetchChunk {
                sequence: 0,
                bytes: vec![1, 2],
            },
        )
        .unwrap();
    client
        .apply(
            Direction::Receive,
            7,
            &Message::FetchCommit { chunk_count: 1 },
        )
        .unwrap();
    assert_eq!(
        client.apply(Direction::Send, 7, &Message::HeadRequest),
        Err(ProtocolError::DuplicateRequestId)
    );
    assert_eq!(
        client.apply(Direction::Send, 6, &Message::HeadRequest),
        Err(ProtocolError::DuplicateRequestId)
    );

    client
        .apply(
            Direction::Send,
            8,
            &Message::UploadBegin {
                expected: None,
                total_length: 1,
                digest: [3; 32],
            },
        )
        .unwrap();
    client
        .apply(
            Direction::Send,
            8,
            &Message::UploadChunk {
                sequence: 0,
                bytes: vec![1],
            },
        )
        .unwrap();
    assert_eq!(
        client.apply(
            Direction::Send,
            8,
            &Message::UploadCommit { chunk_count: 2 }
        ),
        Err(ProtocolError::InvalidState)
    );
    client
        .apply(
            Direction::Send,
            8,
            &Message::UploadCommit { chunk_count: 1 },
        )
        .unwrap();
    client
        .apply(
            Direction::Receive,
            8,
            &Message::UploadAccepted {
                version: version(4),
            },
        )
        .unwrap();
}

// LNS-STATE-002b
#[test]
fn protocol_cancel_is_valid_only_during_chunk_streaming() {
    let mut client = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
    client.authenticate();
    client
        .apply(Direction::Send, 1, &Message::FetchRequest { known: None })
        .unwrap();
    assert_eq!(
        client.apply(Direction::Send, 1, &Message::Cancel),
        Err(ProtocolError::InvalidState)
    );
    client
        .apply(
            Direction::Receive,
            1,
            &Message::FetchBegin {
                version: version(1),
                total_length: 10,
                digest: [2; 32],
            },
        )
        .unwrap();
    client.apply(Direction::Send, 1, &Message::Cancel).unwrap();
    assert_eq!(
        client.apply(
            Direction::Receive,
            1,
            &Message::FetchChunk {
                sequence: 0,
                bytes: vec![0; 10],
            }
        ),
        Err(ProtocolError::InvalidState)
    );
}

// LNS-STATE-003
#[test]
fn protocol_pairing_order_and_transcript_binding_are_strict() {
    let mut pair = ProtocolState::new(SessionMode::Pairing, LocalRole::Client);
    pair.authenticate();
    pair.apply(
        Direction::Send,
        9,
        &Message::PairCommit {
            transaction_id: [1; 16],
            transcript_digest: [2; 32],
        },
    )
    .unwrap();
    assert_eq!(
        pair.apply(
            Direction::Receive,
            9,
            &Message::PairActivated {
                transaction_id: [1; 16],
                transcript_digest: [2; 32]
            }
        ),
        Err(ProtocolError::InvalidState)
    );
    pair.apply(
        Direction::Receive,
        9,
        &Message::PairPrepared {
            transaction_id: [1; 16],
            transcript_digest: [2; 32],
        },
    )
    .unwrap();
}

// LNS-PROTO-004
#[test]
fn protocol_errors_are_secret_free_and_stable() {
    for error in [
        ProtocolError::Malformed,
        ProtocolError::TooLarge,
        ProtocolError::InvalidState,
        ProtocolError::DuplicateRequestId,
        ProtocolError::NotAuthorized,
    ] {
        let text = error.to_string();
        assert!(!text.contains('['));
        assert!(!text.contains("payload"));
    }

    let secret_marker = b"DO-NOT-LOG-THIS".to_vec();
    let debug = format!(
        "{:?}",
        Message::UploadChunk {
            sequence: 0,
            bytes: secret_marker,
        }
    );
    assert_eq!(debug, "Message::UploadChunk([REDACTED])");
    assert_eq!(format!("{:?}", version(0xaa)), "RemoteVersion([REDACTED])");
}
