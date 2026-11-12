//! Core filtering and legacy/manual controls for typed desktop discovery.
use super::*;
use crate::dto::DiscoveryServiceDto;
use crate::fixtures::TestEnv;
use hidlins_sync::discovery::{
    DiscoveryBatch, DiscoveryError, DiscoveryEvent, DiscoveryPort, RawDiscoveryRecord, RawEndpoint,
};
use std::collections::VecDeque;
use std::time::Instant;

fn endpoint(last: u8, port: u16) -> LocalEndpoint {
    LocalEndpoint::new(std::net::Ipv4Addr::new(192, 168, 1, last).into(), port, 0).unwrap()
}

fn record(kind: ServiceKind, tag: u8, route: LocalEndpoint) -> RawDiscoveryRecord {
    RawDiscoveryRecord::try_from_untrusted(
        kind.service_type().to_string(),
        format!("0000000000000000000000000{tag}.{}", kind.service_type()),
        route.port(),
        vec![("v".to_string(), b"1".to_vec())],
        vec![RawEndpoint::new(route.ip(), route.scope_id())],
        Duration::from_secs(120),
    )
    .unwrap()
}

fn populated() -> (TestEnv, AppSession) {
    let env = TestEnv::new();
    let session = AppSession::for_test(env.paths.clone()).unwrap();
    {
        let mut state = session.lock_state();
        state.discovery_permission = DiscoveryPermission::Granted;
        let now = Instant::now();
        state
            .discovery_cache
            .apply_resolved(record(ServiceKind::Trusted, 1, endpoint(10, 48101)), now)
            .unwrap();
        state
            .discovery_cache
            .apply_resolved(record(ServiceKind::Pairing, 2, endpoint(20, 48102)), now)
            .unwrap();
    }
    (env, session)
}

#[test]
fn typed_status_separates_kinds_and_legacy_status_stays_trusted() {
    let (_env, session) = populated();
    let trusted = session.local_discovery_status_for(DiscoveryServiceDto::Trusted);
    let pairing = session.local_discovery_status_for(DiscoveryServiceDto::Pairing);
    assert_eq!(trusted.permission, DiscoveryPermissionDto::Granted);
    assert_eq!(pairing.permission, DiscoveryPermissionDto::Granted);
    assert_eq!(trusted.candidates, vec![endpoint_dto(endpoint(10, 48101))]);
    assert_eq!(pairing.candidates, vec![endpoint_dto(endpoint(20, 48102))]);
    assert_eq!(session.local_discovery_status(), trusted);
}

#[test]
fn explicit_routes_keep_priority_and_deduplicate_for_either_kind() {
    let (_env, session) = populated();
    let manual = endpoint_dto(endpoint(30, 48103));
    let pairing = endpoint_dto(endpoint(20, 48102));
    session
        .set_discovery_candidates(
            DiscoveryPermissionDto::Granted,
            vec![manual.clone(), pairing.clone()],
        )
        .unwrap();
    assert_eq!(
        session
            .local_discovery_status_for(DiscoveryServiceDto::Pairing)
            .candidates,
        vec![manual.clone(), pairing.clone()]
    );
    assert_eq!(
        session
            .local_discovery_status_for(DiscoveryServiceDto::Trusted)
            .candidates,
        vec![manual, pairing, endpoint_dto(endpoint(10, 48101))]
    );
}

struct BatchPort(VecDeque<DiscoveryBatch>);

impl DiscoveryPort for BatchPort {
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
        self.0.pop_front().map_or_else(
            || DiscoveryBatch::new(DiscoveryPermission::Granted, Vec::new()),
            Ok,
        )
    }

    fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        self.0.clear();
        Ok(())
    }
}

#[test]
fn typed_pairing_poll_preserves_trusted_records_for_legacy_poll() {
    let env = TestEnv::new();
    let session = AppSession::for_test(env.paths.clone()).unwrap();
    let batch = DiscoveryBatch::new(
        DiscoveryPermission::Granted,
        vec![
            DiscoveryEvent::Resolved(record(ServiceKind::Trusted, 1, endpoint(10, 48101))),
            DiscoveryEvent::Resolved(record(ServiceKind::Pairing, 2, endpoint(20, 48102))),
        ],
    )
    .unwrap();
    *session.discovery_port.lock().unwrap() = Some(Box::new(BatchPort(VecDeque::from([batch]))));
    assert_eq!(
        session
            .poll_local_discovery_for(DiscoveryServiceDto::Pairing)
            .unwrap()
            .candidates,
        vec![endpoint_dto(endpoint(20, 48102))]
    );
    assert_eq!(
        session.poll_local_discovery().unwrap().candidates,
        vec![endpoint_dto(endpoint(10, 48101))]
    );
}
