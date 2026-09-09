use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};

use hidlins_core::{KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{ClientPairingSession, LanError, LanTransport},
    discovery::ServiceKind,
    framing::{Preface, PrefaceMode},
    identity::PublicIdentity,
    noise::{HandshakeSession, NoiseKeypair, SecureTransport, SessionMode},
    pairing::{ConfirmedPairing, PairingTransaction, SasCode},
    protocol::RemoteVersion,
    protocol::{
        Direction, LocalRole, Message, ProtocolState, MAX_CONNECTIONS_PER_VAULT,
        MAX_PENDING_HOST_OPERATIONS,
    },
    server::{
        AuthoritativeVault, CanonicalSnapshot, HostOperation, HostQueue, HostResponse,
        PairingAuthority, PairingAuthorityError, PendingHostOperation, ServerController,
        ServerError, ServerFaultPlan, ServerFaultPoint, StagedUpload,
    },
    SyncTransport,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn password() -> MasterPassword {
    MasterPassword::new("server integration password".to_string())
}

fn kdf() -> KdfParams {
    KdfParams {
        memory_kib: 1024,
        iterations: 1,
        parallelism: 1,
    }
}

fn create_vault(directory: &TempDir, name: &str) -> Vault {
    Vault::create(
        &directory.path().join(format!("{name}.kdbx")),
        &password(),
        None,
        kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap()
}

fn unused_loopback() -> LocalEndpoint {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    LocalEndpoint::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port, 0).unwrap()
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8]) {
    let length = u16::try_from(bytes.len()).unwrap();
    stream.write_all(&length.to_be_bytes()).unwrap();
    stream.write_all(bytes).unwrap();
}

fn read_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut prefix = [0_u8; 2];
    stream.read_exact(&mut prefix)?;
    let length = usize::from(u16::from_be_bytes(prefix));
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn trusted_connection(
    endpoint: LocalEndpoint,
    client: &NoiseKeypair,
    server_key: [u8; 32],
) -> Result<(TcpStream, SecureTransport, ProtocolState), ()> {
    let mut stream = TcpStream::connect(endpoint.socket_addr()).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    stream
        .write_all(&Preface::new(PrefaceMode::Trusted).encode())
        .map_err(|_| ())?;
    let mut handshake = HandshakeSession::trusted_initiator(client, server_key).map_err(|_| ())?;
    let mut output = vec![0_u8; u16::MAX as usize].into_boxed_slice();
    let length = handshake
        .write_handshake(&[], &mut output)
        .map_err(|_| ())?;
    write_frame(&mut stream, &output[..length]);
    let response = read_frame(&mut stream).map_err(|_| ())?;
    handshake
        .read_handshake(&response, &mut output)
        .map_err(|_| ())?;
    let transport = handshake.finish().map_err(|_| ())?;
    let mut protocol = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
    protocol.authenticate();
    Ok((stream, transport, protocol))
}

fn send_application(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    protocol: &mut ProtocolState,
    request_id: u32,
    message: &Message,
) {
    protocol
        .apply(Direction::Send, request_id, message)
        .unwrap();
    let plain = message.encode(request_id).unwrap();
    let mut encrypted = vec![0_u8; plain.len() + 16];
    let length = transport.write_message(&plain, &mut encrypted).unwrap();
    write_frame(stream, &encrypted[..length]);
}

fn receive_application(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    protocol: &mut ProtocolState,
) -> Message {
    let encrypted = read_frame(stream).unwrap();
    let mut plain = vec![0_u8; encrypted.len()];
    let length = transport.read_message(&encrypted, &mut plain).unwrap();
    let decoded = Message::decode(&plain[..length]).unwrap();
    protocol
        .apply(Direction::Receive, decoded.request_id, &decoded.message)
        .unwrap();
    decoded.message
}

fn take_pending(processor: &mut hidlins_sync::server::HostProcessor) -> PendingHostOperation {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(operation) = processor.take_one().unwrap() {
            return operation;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "network worker did not enqueue its host operation"
        );
        thread::yield_now();
    }
}

fn snapshot_bytes(mut snapshot: CanonicalSnapshot) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(usize::try_from(snapshot.length()).unwrap());
    let mut chunk = [0_u8; 4096];
    loop {
        let read = snapshot.read_chunk(&mut chunk).unwrap();
        if read == 0 {
            return bytes;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[derive(Default)]
struct BlockingPairingAuthority {
    calls: AtomicUsize,
    cancellation: (Mutex<bool>, Condvar),
}

impl BlockingPairingAuthority {
    fn wait_for_calls(&self, expected: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while self.calls.load(Ordering::Acquire) < expected {
            assert!(
                std::time::Instant::now() < deadline,
                "pairing prompt was not reached"
            );
            thread::yield_now();
        }
    }
}

impl PairingAuthority for BlockingPairingAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        _sas: SasCode,
        _timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        let (cancelled, wake) = &self.cancellation;
        let mut cancelled = cancelled.lock().unwrap();
        while !*cancelled {
            cancelled = wake.wait(cancelled).unwrap();
        }
        Err(PairingAuthorityError::Canceled)
    }

    fn prepare(
        &self,
        _transaction: &PairingTransaction,
        _confirmation: &ConfirmedPairing,
        _display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        Ok(())
    }

    fn activate(&self, _transaction: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        Ok(())
    }

    fn cancel_pending(&self) {
        let (cancelled, wake) = &self.cancellation;
        *cancelled.lock().unwrap() = true;
        wake.notify_all();
    }

    fn shutdown(&self) {
        self.cancel_pending();
    }
}

// LNS-SERVER-001
#[test]
fn host_head_fetch_and_stale_cas_are_canonical_and_serialized() {
    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "server");
    let master = password();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);

    let first = host.head().unwrap();
    let snapshot = host.fetch(None).unwrap().unwrap();
    assert_eq!(snapshot.version(), first);
    let current = snapshot_bytes(snapshot);
    assert_eq!(RemoteVersion::new(Sha256::digest(&current).into()), first);
    assert!(host.fetch(Some(first)).unwrap().is_none());

    let client_dir = TempDir::new().unwrap();
    let client = create_vault(&client_dir, "other-root");
    let foreign = std::fs::read(client.path()).unwrap();
    let digest: [u8; 32] = Sha256::digest(&foreign).into();
    let mut stage =
        StagedUpload::new(host.path(), Some(first), foreign.len() as u64, digest).unwrap();
    stage.write_chunk(0, &foreign).unwrap();
    let stage = stage.finish(1).unwrap();
    assert_eq!(host.commit(&stage), Err(ServerError::InvalidUpload));
    assert_eq!(host.head().unwrap(), first);

    let digest: [u8; 32] = Sha256::digest(&current).into();
    let mut stale_upload = StagedUpload::new(
        host.path(),
        Some(RemoteVersion::new([7; 32])),
        current.len() as u64,
        digest,
    )
    .unwrap();
    stale_upload.write_chunk(0, &current).unwrap();
    let stale_upload = stale_upload.finish(1).unwrap();
    assert_eq!(host.commit(&stale_upload), Err(ServerError::StaleVersion));
    assert_eq!(host.head().unwrap(), first);
}

// LNS-SERVER-007
#[test]
fn simultaneous_client_commits_are_serialized_and_exactly_one_cas_wins() {
    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "cas-race");
    let original = std::fs::read(vault.path()).unwrap();
    let expected = RemoteVersion::new(Sha256::digest(&original).into());
    let digest = *expected.as_bytes();

    let stage = || {
        let mut upload =
            StagedUpload::new(vault.path(), Some(expected), original.len() as u64, digest).unwrap();
        upload.write_chunk(0, &original).unwrap();
        upload.finish(1).unwrap()
    };
    let first_stage = stage();
    let second_stage = stage();
    let (queue, mut processor) = HostQueue::new();
    let first_queue = queue.clone();
    let first = thread::spawn(move || {
        first_queue
            .submit(HostOperation::Commit(first_stage))
            .unwrap()
    });
    let second = thread::spawn(move || queue.submit(HostOperation::Commit(second_stage)).unwrap());
    let first = first.join().unwrap();
    let second = second.join().unwrap();

    let master = password();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    processor.process_one(&mut host).unwrap();
    processor.process_one(&mut host).unwrap();
    let outcomes = [
        first.wait_timeout(Duration::from_secs(1)),
        second.wait_timeout(Duration::from_secs(1)),
    ];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Ok(HostResponse::Committed(_))))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Err(ServerError::StaleVersion)))
            .count(),
        1
    );
    assert_ne!(host.head().unwrap(), expected);
    let path = host.path().to_path_buf();
    drop(host);
    drop(vault);
    Vault::open(&path, &password(), None).unwrap();
}

// LNS-SERVER-002
#[test]
fn host_queue_is_bounded_cancellable_and_shutdown_resolves_waiters() {
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Head).unwrap();
    ticket.cancel();

    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "server");
    let master = password();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    processor.process_one(&mut host).unwrap();
    assert!(matches!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Err(ServerError::Cancelled)
    ));

    let pending: Vec<_> = (0..MAX_PENDING_HOST_OPERATIONS)
        .map(|_| queue.submit(HostOperation::Head).unwrap())
        .collect();
    assert!(matches!(
        queue.submit(HostOperation::Head),
        Err(ServerError::Busy)
    ));
    processor.shutdown();
    for pending_ticket in pending {
        assert!(matches!(
            pending_ticket.wait_timeout(Duration::from_secs(1)),
            Err(ServerError::Stopped)
        ));
    }
    assert!(matches!(
        queue.submit(HostOperation::Head),
        Err(ServerError::Stopped)
    ));
}

// LNS-SERVER-003
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end session deliberately proves head, fetch, commit, and silent rejection"
)]
fn real_loopback_noise_allows_active_key_and_silently_rejects_other_keys() {
    let directory = TempDir::new().unwrap();
    let vault = create_vault(&directory, "network-server");
    let expected_bytes = std::fs::read(vault.path()).unwrap();
    let expected_version = RemoteVersion::new(Sha256::digest(&expected_bytes).into());
    let master = password();
    let (queue, mut processor) = HostQueue::new();
    let host_running = Arc::new(AtomicBool::new(true));
    let thread_running = Arc::clone(&host_running);
    let host_thread = thread::spawn(move || {
        let mut vault = vault;
        let mut host = AuthoritativeVault::new(&mut vault, &master, None);
        while thread_running.load(Ordering::Acquire) {
            if !processor.process_one(&mut host).unwrap_or(false) {
                thread::yield_now();
            }
        }
        processor.shutdown();
    });

    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let client_identity = NoiseKeypair::generate().unwrap();
    let client_public = client_identity.public_key();
    let rejected_identity = NoiseKeypair::generate().unwrap();
    let mut server = ServerController::start(
        unused_loopback(),
        Arc::clone(&server_identity),
        [client_public],
        queue,
    )
    .unwrap();

    let (mut stream, mut transport, mut protocol) = trusted_connection(
        server.endpoint(),
        &client_identity,
        server_identity.public_key(),
    )
    .unwrap();
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        1,
        &Message::HeadRequest,
    );
    assert_eq!(
        receive_application(&mut stream, &mut transport, &mut protocol),
        Message::HeadResponse {
            version: Some(expected_version)
        }
    );

    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        2,
        &Message::FetchRequest { known: None },
    );
    assert!(matches!(
        receive_application(&mut stream, &mut transport, &mut protocol),
        Message::FetchBegin {
            version,
            total_length,
            digest,
        } if version == expected_version
            && total_length == expected_bytes.len() as u64
            && digest == *expected_version.as_bytes()
    ));
    let fetched = match receive_application(&mut stream, &mut transport, &mut protocol) {
        Message::FetchChunk { sequence: 0, bytes } => bytes,
        other => panic!("unexpected fetch record: {other:?}"),
    };
    assert_eq!(fetched, expected_bytes);
    assert_eq!(
        receive_application(&mut stream, &mut transport, &mut protocol),
        Message::FetchCommit { chunk_count: 1 }
    );

    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        3,
        &Message::UploadBegin {
            expected: Some(expected_version),
            total_length: expected_bytes.len() as u64,
            digest: *expected_version.as_bytes(),
        },
    );
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        3,
        &Message::UploadChunk {
            sequence: 0,
            bytes: expected_bytes.clone(),
        },
    );
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        3,
        &Message::UploadCommit { chunk_count: 1 },
    );
    assert!(matches!(
        receive_application(&mut stream, &mut transport, &mut protocol),
        Message::UploadAccepted { version } if version != expected_version
    ));
    drop(stream);

    let rejected = trusted_connection(
        server.endpoint(),
        &rejected_identity,
        server_identity.public_key(),
    );
    let (mut stream, mut transport, mut protocol) = rejected.unwrap();
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        1,
        &Message::HeadRequest,
    );
    assert!(read_frame(&mut stream).is_err());

    server.replace_trusted([]);
    drop(stream);
    server.stop();
    assert!(server.advertisements().is_empty());
    host_running.store(false, Ordering::Release);
    host_thread.join().unwrap();
}

// LNS-SERVER-003, LNS-SEC-012
// LNS-REVIEW-001
#[test]
fn trust_replacement_closes_only_removed_live_peers() {
    let directory = TempDir::new().unwrap();
    let vault = create_vault(&directory, "live-revocation");
    let expected = RemoteVersion::new(Sha256::digest(std::fs::read(vault.path()).unwrap()).into());
    let master = password();
    let (queue, mut processor) = HostQueue::new();
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = Arc::clone(&running);
    let host_thread = thread::spawn(move || {
        let mut vault = vault;
        let mut host = AuthoritativeVault::new(&mut vault, &master, None);
        while thread_running.load(Ordering::Acquire) {
            if !processor.process_one(&mut host).unwrap_or(false) {
                thread::yield_now();
            }
        }
    });

    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let removed = NoiseKeypair::generate().unwrap();
    let retained = NoiseKeypair::generate().unwrap();
    let mut server = ServerController::start(
        unused_loopback(),
        Arc::clone(&server_identity),
        [removed.public_key(), retained.public_key()],
        queue,
    )
    .unwrap();
    let (mut removed_stream, mut removed_transport, mut removed_protocol) =
        trusted_connection(server.endpoint(), &removed, server_identity.public_key()).unwrap();
    let (mut retained_stream, mut retained_transport, mut retained_protocol) =
        trusted_connection(server.endpoint(), &retained, server_identity.public_key()).unwrap();

    server.replace_trusted([retained.public_key()]);
    send_application(
        &mut removed_stream,
        &mut removed_transport,
        &mut removed_protocol,
        1,
        &Message::HeadRequest,
    );
    assert!(read_frame(&mut removed_stream).is_err());

    send_application(
        &mut retained_stream,
        &mut retained_transport,
        &mut retained_protocol,
        1,
        &Message::HeadRequest,
    );
    assert_eq!(
        receive_application(
            &mut retained_stream,
            &mut retained_transport,
            &mut retained_protocol,
        ),
        Message::HeadResponse {
            version: Some(expected),
        }
    );

    server.stop();
    running.store(false, Ordering::Release);
    host_thread.join().unwrap();
}

// LNS-SERVER-003, LNS-SEC-012
// LNS-REVIEW-001
#[test]
fn revocation_cancels_a_dequeued_commit_before_execution() {
    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "queued-revocation");
    let original = std::fs::read(vault.path()).unwrap();
    let original_version = RemoteVersion::new(Sha256::digest(&original).into());
    let master = password();
    let (queue, mut processor) = HostQueue::new();
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let client_identity = NoiseKeypair::generate().unwrap();
    let mut server = ServerController::start(
        unused_loopback(),
        Arc::clone(&server_identity),
        [client_identity.public_key()],
        queue,
    )
    .unwrap();
    let (mut stream, mut transport, mut protocol) = trusted_connection(
        server.endpoint(),
        &client_identity,
        server_identity.public_key(),
    )
    .unwrap();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);

    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        7,
        &Message::UploadBegin {
            expected: Some(original_version),
            total_length: original.len() as u64,
            digest: *original_version.as_bytes(),
        },
    );
    take_pending(&mut processor).execute(&mut host).unwrap();
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        7,
        &Message::UploadChunk {
            sequence: 0,
            bytes: original.clone(),
        },
    );
    send_application(
        &mut stream,
        &mut transport,
        &mut protocol,
        7,
        &Message::UploadCommit { chunk_count: 1 },
    );
    let pending_commit = take_pending(&mut processor);

    server.replace_trusted([]);
    pending_commit.execute(&mut host).unwrap();
    assert_eq!(host.head().unwrap(), original_version);
    assert_eq!(std::fs::read(host.path()).unwrap(), original);

    server.stop();
}

// LNS-PAIR-002, LNS-PAIR-003
// LNS-REVIEW-005
#[test]
fn runtime_admits_one_pairing_candidate_and_close_cancels_it() {
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let first_client = NoiseKeypair::generate().unwrap();
    let second_client = NoiseKeypair::generate().unwrap();
    let authority = Arc::new(BlockingPairingAuthority::default());
    let (queue, _processor) = HostQueue::new();
    let mut server = ServerController::start_with_pairing_authority(
        unused_loopback(),
        Arc::clone(&server_identity),
        [],
        queue,
        Some(authority.clone()),
    )
    .unwrap();
    server.open_pairing().unwrap();

    let first = ClientPairingSession::begin(&first_client, [server.endpoint()]).unwrap();
    authority.wait_for_calls(1);
    let second = ClientPairingSession::begin(&second_client, [server.endpoint()]).unwrap();
    thread::sleep(Duration::from_millis(150));
    assert_eq!(authority.calls.load(Ordering::Acquire), 1);

    server.close_pairing();
    assert!(server
        .advertisements()
        .iter()
        .all(|advertisement| advertisement.kind() == ServiceKind::Trusted));
    drop(first);
    drop(second);
    server.stop();
}

// LNS-PAIR-002, LNS-SEC-007
// LNS-REVIEW-005
#[test]
fn three_failed_xx_handshakes_close_pairing_before_a_fourth_attempt() {
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let authority = Arc::new(BlockingPairingAuthority::default());
    let (queue, _processor) = HostQueue::new();
    let mut server = ServerController::start_with_pairing_authority(
        unused_loopback(),
        server_identity,
        [],
        queue,
        Some(authority.clone()),
    )
    .unwrap();
    server.open_pairing().unwrap();

    for _ in 0..3 {
        let mut stream = TcpStream::connect(server.endpoint().socket_addr()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        stream
            .write_all(&Preface::new(PrefaceMode::Pairing).encode())
            .unwrap();
        write_frame(&mut stream, b"not a Noise handshake");
        assert!(read_frame(&mut stream).is_err());
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while server
        .advertisements()
        .iter()
        .any(|advertisement| advertisement.kind() == ServiceKind::Pairing)
    {
        assert!(
            std::time::Instant::now() < deadline,
            "pairing stayed open after failure exhaustion"
        );
        thread::yield_now();
    }
    assert_eq!(authority.calls.load(Ordering::Acquire), 0);

    let mut fourth = TcpStream::connect(server.endpoint().socket_addr()).unwrap();
    fourth
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    fourth
        .write_all(&Preface::new(PrefaceMode::Pairing).encode())
        .unwrap();
    assert!(read_frame(&mut fourth).is_err());
    server.stop();
}

// LNS-SEC-007
// LNS-REVIEW-007
#[test]
fn connection_capacity_recovers_after_rejection_and_disconnect() {
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let client_identity = NoiseKeypair::generate().unwrap();
    let (queue, _processor) = HostQueue::new();
    let mut server = ServerController::start(
        unused_loopback(),
        Arc::clone(&server_identity),
        [client_identity.public_key()],
        queue,
    )
    .unwrap();

    let mut held = Vec::new();
    for _ in 0..MAX_CONNECTIONS_PER_VAULT {
        held.push(TcpStream::connect(server.endpoint().socket_addr()).unwrap());
    }
    thread::sleep(Duration::from_millis(250));
    let mut rejected = TcpStream::connect(server.endpoint().socket_addr()).unwrap();
    rejected
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    assert!(read_frame(&mut rejected).is_err());

    drop(held);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if trusted_connection(
            server.endpoint(),
            &client_identity,
            server_identity.public_key(),
        )
        .is_ok()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "released connection slots were not reusable"
        );
        thread::yield_now();
    }
    server.stop();
}

// LNS-SEC-007
// LNS-REVIEW-006
#[test]
fn slow_preface_peer_is_closed_by_the_noise_handshake_deadline() {
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let (queue, _processor) = HostQueue::new();
    let mut server =
        ServerController::start(unused_loopback(), server_identity, [], queue).unwrap();
    let started = std::time::Instant::now();
    let mut stream = TcpStream::connect(server.endpoint().socket_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(7)))
        .unwrap();
    assert!(read_frame(&mut stream).is_err());
    assert!(started.elapsed() < Duration::from_secs(7));
    server.stop();
}

// LNS-SERVER-001
// LNS-REVIEW-008
#[test]
fn file_backed_fetch_snapshot_remains_coherent_after_live_path_replacement() {
    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "snapshot-coherence");
    let original = std::fs::read(vault.path()).unwrap();
    let original_version = RemoteVersion::new(Sha256::digest(&original).into());
    let mut snapshot = {
        let master = password();
        let host = AuthoritativeVault::new(&mut vault, &master, None);
        host.fetch(None).unwrap().unwrap()
    };

    let replacement_path = directory.path().join("replacement.kdbx");
    let replacement = Vault::create(
        &replacement_path,
        &password(),
        None,
        kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    std::fs::rename(replacement.path(), vault.path()).unwrap();

    let mut streamed = Vec::new();
    let mut bounded = vec![0_u8; hidlins_sync::protocol::MAX_APPLICATION_CHUNK];
    loop {
        let read = snapshot.read_chunk(&mut bounded).unwrap();
        if read == 0 {
            break;
        }
        assert!(read <= hidlins_sync::protocol::MAX_APPLICATION_CHUNK);
        streamed.extend_from_slice(&bounded[..read]);
    }
    assert_eq!(snapshot.version(), original_version);
    assert_eq!(streamed, original);
}

// LNS-SERVER-004
#[test]
fn staging_and_commit_fault_boundaries_preserve_complete_canonical_vault() {
    for point in [
        ServerFaultPoint::StageCreate,
        ServerFaultPoint::StageWrite,
        ServerFaultPoint::StageFinalize,
    ] {
        let directory = TempDir::new().unwrap();
        let vault = create_vault(&directory, "stage-fault");
        let bytes = std::fs::read(vault.path()).unwrap();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let plan = ServerFaultPlan::at([point]);
        let result = StagedUpload::with_faults(
            vault.path(),
            Some(RemoteVersion::new(digest)),
            bytes.len() as u64,
            digest,
            plan,
        );
        match point {
            ServerFaultPoint::StageCreate => assert!(matches!(result, Err(ServerError::Internal))),
            ServerFaultPoint::StageWrite => {
                let mut stage = result.unwrap();
                assert_eq!(stage.write_chunk(0, &bytes), Err(ServerError::Internal));
            }
            ServerFaultPoint::StageFinalize => {
                let mut stage = result.unwrap();
                stage.write_chunk(0, &bytes).unwrap();
                assert!(matches!(stage.finish(1), Err(ServerError::Internal)));
            }
            _ => unreachable!(),
        }
        assert_eq!(std::fs::read(vault.path()).unwrap(), bytes);
        assert_eq!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".hidlins-upload-"))
                .count(),
            0
        );
    }

    for point in [
        ServerFaultPoint::Validation,
        ServerFaultPoint::Backup,
        ServerFaultPoint::DatabaseReplacement,
        ServerFaultPoint::Save,
    ] {
        let directory = TempDir::new().unwrap();
        let mut vault = create_vault(&directory, "commit-fault");
        let original = std::fs::read(vault.path()).unwrap();
        let version = RemoteVersion::new(Sha256::digest(&original).into());
        let digest = *version.as_bytes();
        let mut stage =
            StagedUpload::new(vault.path(), Some(version), original.len() as u64, digest).unwrap();
        stage.write_chunk(0, &original).unwrap();
        let stage = stage.finish(1).unwrap();
        let master = password();
        let mut host = AuthoritativeVault::with_faults(
            &mut vault,
            &master,
            None,
            ServerFaultPlan::at([point]),
        );
        assert_eq!(host.commit(&stage), Err(ServerError::Internal));
        assert_eq!(std::fs::read(host.path()).unwrap(), original);
        assert_eq!(host.head().unwrap(), version);
    }
}

// LNS-SERVER-005
#[test]
fn upload_limits_digest_and_malformed_kdbx_fail_closed() {
    let directory = TempDir::new().unwrap();
    let mut vault = create_vault(&directory, "invalid-upload");
    let original = std::fs::read(vault.path()).unwrap();
    let version = RemoteVersion::new(Sha256::digest(&original).into());
    assert!(matches!(
        StagedUpload::new(vault.path(), Some(version), 0, [0; 32]),
        Err(ServerError::TooLarge)
    ));

    let mut bad_digest = StagedUpload::new(vault.path(), Some(version), 3, [0; 32]).unwrap();
    bad_digest.write_chunk(0, b"bad").unwrap();
    assert!(matches!(
        bad_digest.finish(1),
        Err(ServerError::InvalidUpload)
    ));

    let rekey_path = directory.path().join("wrong-password.kdbx");
    std::fs::write(&rekey_path, &original).unwrap();
    let mut rekeyed = Vault::open(&rekey_path, &password(), None).unwrap();
    rekeyed
        .change_master_password(
            &password(),
            &MasterPassword::new("different password".to_string()),
        )
        .unwrap();
    let rekeyed_bytes = std::fs::read(&rekey_path).unwrap();
    drop(rekeyed);
    let rekeyed_digest: [u8; 32] = Sha256::digest(&rekeyed_bytes).into();
    let mut wrong_credential = StagedUpload::new(
        vault.path(),
        Some(version),
        rekeyed_bytes.len() as u64,
        rekeyed_digest,
    )
    .unwrap();
    wrong_credential.write_chunk(0, &rekeyed_bytes).unwrap();
    let wrong_credential = wrong_credential.finish(1).unwrap();
    let master = password();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    assert_eq!(
        host.commit(&wrong_credential),
        Err(ServerError::InvalidUpload)
    );
    drop(host);

    let malformed = b"not a kdbx";
    let digest: [u8; 32] = Sha256::digest(malformed).into();
    let mut stage =
        StagedUpload::new(vault.path(), Some(version), malformed.len() as u64, digest).unwrap();
    stage.write_chunk(0, malformed).unwrap();
    let stage = stage.finish(1).unwrap();
    let master = password();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    assert_eq!(host.commit(&stage), Err(ServerError::InvalidUpload));
    assert_eq!(std::fs::read(host.path()).unwrap(), original);
}

// LNS-SERVER-006
// LNS-REVIEW-009
#[test]
fn client_falls_back_from_forged_route_but_never_accepts_a_wrong_pin() {
    let directory = TempDir::new().unwrap();
    let vault = create_vault(&directory, "client-server");
    let master = password();
    let (queue, mut processor) = HostQueue::new();
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = Arc::clone(&running);
    let host_thread = thread::spawn(move || {
        let mut vault = vault;
        let mut host = AuthoritativeVault::new(&mut vault, &master, None);
        while thread_running.load(Ordering::Acquire) {
            if !processor.process_one(&mut host).unwrap_or(false) {
                thread::yield_now();
            }
        }
    });
    let server_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let client_identity = NoiseKeypair::generate().unwrap();
    let client_public = client_identity.public_key();
    let mut server = ServerController::start(
        unused_loopback(),
        Arc::clone(&server_identity),
        [client_public],
        queue,
    )
    .unwrap();
    let spoof_identity = Arc::new(NoiseKeypair::generate().unwrap());
    let (spoof_queue, _spoof_processor) = HostQueue::new();
    let mut spoof = ServerController::start(
        unused_loopback(),
        spoof_identity,
        [client_public],
        spoof_queue,
    )
    .unwrap();
    let obsolete_route = unused_loopback();
    let mut client = LanTransport::new(
        client_identity,
        server_identity.public_key(),
        Some(obsolete_route),
        [server.endpoint()],
    )
    .unwrap();
    let head = client.head().unwrap().unwrap();
    assert!(client.fetch_if_changed(Some(&head)).unwrap().is_none());
    let snapshot = client.fetch_if_changed(None).unwrap().unwrap();
    let committed = client
        .commit_conditional(&snapshot.bytes, Some(&head))
        .unwrap();
    assert_ne!(committed, head);
    assert_eq!(
        client
            .commit_conditional(&snapshot.bytes, Some(&head))
            .unwrap_err(),
        LanError::StaleVersion
    );

    let fallback_identity = NoiseKeypair::generate().unwrap();
    server.replace_trusted([client_public, fallback_identity.public_key()]);
    let mut fallback = LanTransport::new(
        fallback_identity,
        server_identity.public_key(),
        Some(spoof.endpoint()),
        [server.endpoint()],
    )
    .unwrap();
    assert!(fallback.head().unwrap().is_some());

    let wrong_pin = NoiseKeypair::generate().unwrap().public_key();
    let mut spoofed = LanTransport::new(
        NoiseKeypair::generate().unwrap(),
        wrong_pin,
        Some(server.endpoint()),
        [],
    )
    .unwrap();
    assert_eq!(spoofed.head().unwrap_err(), LanError::Authentication);

    spoof.stop();
    server.stop();
    running.store(false, Ordering::Release);
    host_thread.join().unwrap();
}
