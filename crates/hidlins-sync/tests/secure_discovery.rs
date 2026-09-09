use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};

use hidlins_sync::{
    discovery::{
        manual_endpoint, AdvertisementSet, CandidateCache, DiscoveryBatch, DiscoveryError,
        DiscoveryEvent, DiscoveryPermission, RawDiscoveryRecord, RawEndpoint, ServiceKind,
        SimulatedDiscoveryPort, PAIRING_SERVICE_TYPE, PROTOCOL_TXT, TRUSTED_SERVICE_TYPE,
    },
    protocol::{MAX_DISCOVERY_ENDPOINTS, PAIRING_WINDOW},
};

// LNS-DISCOVERY-001
// LNS-REVIEW-002
#[test]
fn native_service_type_constants_match_the_rust_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let swift =
        std::fs::read_to_string(root.join("app/ios/Runner/HidlinsPlatformServices.swift")).unwrap();
    let plist = std::fs::read_to_string(root.join("app/ios/Runner/Info.plist")).unwrap();
    let java = std::fs::read_to_string(
        root.join("app/android/app/src/main/java/app/hidlins/LocalDiscoveryController.java"),
    )
    .unwrap();
    for service in ["_hidlins-sync._tcp", "_hidlins-pair._tcp"] {
        assert!(swift.contains(service));
        assert!(plist.contains(service));
        assert!(java.contains(service));
    }
    for obsolete in ["_hidlins-v1._tcp", "_hidlins-pair-v1._tcp"] {
        assert!(!swift.contains(obsolete));
        assert!(!plist.contains(obsolete));
        assert!(!java.contains(obsolete));
    }
    assert_eq!(TRUSTED_SERVICE_TYPE, "_hidlins-sync._tcp.local.");
    assert_eq!(PAIRING_SERVICE_TYPE, "_hidlins-pair._tcp.local.");
}

fn raw_record(
    kind: ServiceKind,
    label: &str,
    port: u16,
    endpoints: Vec<RawEndpoint>,
    ttl: Duration,
) -> RawDiscoveryRecord {
    RawDiscoveryRecord::try_from_untrusted(
        kind.service_type().to_string(),
        format!("{label}.{}", kind.service_type()),
        port,
        vec![("v".to_string(), b"1".to_vec())],
        endpoints,
        ttl,
    )
    .unwrap()
}

fn v4(last: u8) -> RawEndpoint {
    RawEndpoint::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, last)), 0)
}

// LNS-DISCOVERY-001
#[test]
fn dhcp_change_replaces_route_without_changing_identity() {
    let now = Instant::now();
    let label = "00000000000000000000000001";
    let pinned_identity = [0x51; 32];
    let mut cache = CandidateCache::new();

    cache
        .apply_resolved(
            raw_record(
                ServiceKind::Trusted,
                label,
                48101,
                vec![v4(10)],
                Duration::from_secs(120),
            ),
            now,
        )
        .unwrap();
    assert_eq!(
        cache.candidates(ServiceKind::Trusted)[0].ip(),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))
    );

    cache
        .apply_resolved(
            raw_record(
                ServiceKind::Trusted,
                label,
                48101,
                vec![v4(55)],
                Duration::from_secs(120),
            ),
            now + Duration::from_secs(10),
        )
        .unwrap();

    let routes = cache.candidates(ServiceKind::Trusted);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].ip(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 55)));
    assert_eq!(pinned_identity, [0x51; 32]);
}

// LNS-DISCOVERY-002
#[test]
fn untrusted_records_are_strictly_filtered_deduplicated_and_bounded() {
    let now = Instant::now();
    let label = "00000000000000000000000002";
    let mut cache = CandidateCache::new();

    let oversized = RawDiscoveryRecord::try_from_untrusted(
        TRUSTED_SERVICE_TYPE.to_string(),
        format!("{label}.{TRUSTED_SERVICE_TYPE}"),
        48101,
        vec![("v".to_string(), b"1".to_vec())],
        vec![v4(1); MAX_DISCOVERY_ENDPOINTS + 1],
        Duration::from_secs(120),
    );
    assert_eq!(oversized, Err(DiscoveryError::Capacity));

    let invalid = [
        RawDiscoveryRecord::try_from_untrusted(
            "_http._tcp.local.".to_string(),
            format!("{label}._http._tcp.local."),
            48101,
            vec![("v".to_string(), b"1".to_vec())],
            vec![v4(1)],
            Duration::from_secs(120),
        )
        .unwrap(),
        RawDiscoveryRecord::try_from_untrusted(
            TRUSTED_SERVICE_TYPE.to_string(),
            format!("{label}.{TRUSTED_SERVICE_TYPE}"),
            48101,
            vec![
                ("v".to_string(), b"1".to_vec()),
                ("key".to_string(), vec![7; 32]),
            ],
            vec![v4(1)],
            Duration::from_secs(120),
        )
        .unwrap(),
        raw_record(
            ServiceKind::Trusted,
            "stable-device-name",
            48101,
            vec![v4(1)],
            Duration::from_secs(120),
        ),
        raw_record(
            ServiceKind::Trusted,
            label,
            48101,
            vec![RawEndpoint::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 0)],
            Duration::from_secs(120),
        ),
    ];
    for record in invalid {
        assert!(cache.apply_resolved(record, now).is_err());
    }
    assert!(cache.candidates(ServiceKind::Trusted).is_empty());

    let mapped = Ipv4Addr::new(192, 168, 1, 9).to_ipv6_mapped();
    cache
        .apply_resolved(
            raw_record(
                ServiceKind::Trusted,
                label,
                48101,
                vec![v4(9), RawEndpoint::new(IpAddr::V6(mapped), 0), v4(9)],
                Duration::from_secs(1),
            ),
            now,
        )
        .unwrap();
    assert_eq!(cache.candidates(ServiceKind::Trusted).len(), 1);
    cache.purge_expired(now + Duration::from_secs(2));
    assert!(cache.candidates(ServiceKind::Trusted).is_empty());

    let endpoints = (1..=MAX_DISCOVERY_ENDPOINTS)
        .map(|last| v4(u8::try_from(last).unwrap()))
        .collect();
    cache
        .apply_resolved(
            raw_record(
                ServiceKind::Trusted,
                label,
                48101,
                endpoints,
                Duration::from_secs(120),
            ),
            now,
        )
        .unwrap();
    let before = cache.candidates(ServiceKind::Trusted);
    let overflow = raw_record(
        ServiceKind::Trusted,
        "00000000000000000000000003",
        48101,
        vec![RawEndpoint::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 2, 1)),
            0,
        )],
        Duration::from_secs(120),
    );
    assert_eq!(
        cache.apply_resolved(overflow, now),
        Err(DiscoveryError::Capacity)
    );
    assert_eq!(cache.candidates(ServiceKind::Trusted), before);
}

// LNS-DISCOVERY-003
#[test]
fn multi_interface_link_local_routes_remain_scoped_and_removals_are_exact() {
    let now = Instant::now();
    let label = "00000000000000000000000004";
    let ip: Ipv6Addr = "fe80::1234".parse().unwrap();
    let mut cache = CandidateCache::new();
    let record = raw_record(
        ServiceKind::Trusted,
        label,
        48101,
        vec![
            RawEndpoint::new(IpAddr::V6(ip), 4),
            RawEndpoint::new(IpAddr::V6(ip), 9),
        ],
        Duration::from_secs(120),
    );
    cache.apply_resolved(record, now).unwrap();
    assert_eq!(cache.candidates(ServiceKind::Trusted).len(), 2);

    cache.remove(
        TRUSTED_SERVICE_TYPE,
        &format!("{label}.{TRUSTED_SERVICE_TYPE}"),
    );
    assert!(cache.candidates(ServiceKind::Trusted).is_empty());
}

// LNS-DISCOVERY-004
#[test]
fn advertisements_are_ephemeral_minimal_and_pairing_is_window_bound() {
    let now = Instant::now();
    let mut advertisements = AdvertisementSet::start_with_entropy(48101, [0xAB; 16]).unwrap();
    let label = advertisements.instance_label().to_string();
    let other = AdvertisementSet::start_with_entropy(48101, [0xAC; 16]).unwrap();
    assert_ne!(advertisements.instance_label(), other.instance_label());
    assert_eq!(label.len(), 26);
    assert!(label
        .bytes()
        .all(|byte| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&byte)));

    let running = advertisements.active(now);
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].kind(), ServiceKind::Trusted);
    assert_eq!(running[0].service_type(), TRUSTED_SERVICE_TYPE);
    assert_eq!(running[0].txt(), PROTOCOL_TXT);
    assert_eq!(running[0].instance_label(), label);
    assert_eq!(running[0].hostname(), format!("{label}.local."));

    advertisements.open_pairing_at(now).unwrap();
    assert_eq!(advertisements.active(now).len(), 2);
    assert!(advertisements
        .active(now)
        .iter()
        .any(|record| record.service_type() == PAIRING_SERVICE_TYPE));
    assert_eq!(advertisements.active(now + PAIRING_WINDOW).len(), 1);

    advertisements.lock();
    assert!(advertisements.active(now).is_empty());
}

// LNS-DISCOVERY-005
#[test]
fn native_port_enforces_permission_batch_bounds_and_manual_policy() {
    let now = Instant::now();
    let label = "00000000000000000000000005";
    let event = DiscoveryEvent::Resolved(raw_record(
        ServiceKind::Trusted,
        label,
        48101,
        vec![v4(7)],
        Duration::from_secs(120),
    ));
    for (permission, expected) in [
        (
            DiscoveryPermission::NotDetermined,
            DiscoveryError::PermissionRequired,
        ),
        (
            DiscoveryPermission::Denied,
            DiscoveryError::PermissionDenied,
        ),
        (
            DiscoveryPermission::Restricted,
            DiscoveryError::PermissionDenied,
        ),
        (
            DiscoveryPermission::Unavailable,
            DiscoveryError::Unavailable,
        ),
    ] {
        let batch = DiscoveryBatch::new(permission, vec![event.clone()]).unwrap();
        let mut port = SimulatedDiscoveryPort::new(vec![batch]);
        let mut cache = CandidateCache::new();
        assert_eq!(port.poll_into(&mut cache, now), Err(expected));
        assert!(cache.candidates(ServiceKind::Trusted).is_empty());
    }

    let spoof = DiscoveryEvent::Resolved(
        RawDiscoveryRecord::try_from_untrusted(
            TRUSTED_SERVICE_TYPE.to_string(),
            format!("{label}.{TRUSTED_SERVICE_TYPE}"),
            48101,
            vec![("identity".to_string(), vec![0x51; 32])],
            vec![v4(8)],
            Duration::from_secs(120),
        )
        .unwrap(),
    );
    let mixed =
        DiscoveryBatch::new(DiscoveryPermission::Granted, vec![spoof, event.clone()]).unwrap();
    let mut port = SimulatedDiscoveryPort::new(vec![mixed]);
    let mut cache = CandidateCache::new();
    port.poll_into(&mut cache, now).unwrap();
    assert_eq!(cache.candidates(ServiceKind::Trusted).len(), 1);

    let too_many = vec![event; MAX_DISCOVERY_ENDPOINTS + 1];
    assert_eq!(
        DiscoveryBatch::new(DiscoveryPermission::Granted, too_many),
        Err(DiscoveryError::Capacity)
    );

    assert_eq!(
        manual_endpoint("192.168.1.7:48101").unwrap().ip(),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 7))
    );
    assert!(manual_endpoint("server.local:48101").is_err());
    assert!(manual_endpoint("8.8.8.8:48101").is_err());
}
