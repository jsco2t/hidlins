//! Shared initialized Noise and loopback fixtures for deadline regression modules.

#![allow(dead_code)] // The two owning test modules use different fixture subsets.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, Shutdown, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    framing::encode_noise_frame,
    noise::{HandshakeSession, NoiseKeypair, SecureTransport},
    protocol::Message,
};

pub fn sockets() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    for socket in [&client, &server] {
        socket.set_nodelay(true).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
    }
    (client, server)
}

pub fn noise_pair() -> (SecureTransport, SecureTransport) {
    let client = NoiseKeypair::generate().unwrap();
    let server = NoiseKeypair::generate().unwrap();
    let mut initiator = HandshakeSession::trusted_initiator(&client, server.public_key()).unwrap();
    let mut responder = HandshakeSession::trusted_responder_for_authorization(&server).unwrap();
    let mut wire = [0_u8; 512];
    let mut plain = [0_u8; 512];
    let length = initiator.write_handshake(&[], &mut wire).unwrap();
    responder
        .read_handshake(&wire[..length], &mut plain)
        .unwrap();
    let length = responder.write_handshake(&[], &mut wire).unwrap();
    initiator
        .read_handshake(&wire[..length], &mut plain)
        .unwrap();
    (initiator.finish().unwrap(), responder.finish().unwrap())
}

pub fn record(transport: &mut SecureTransport, message: &Message) -> Vec<u8> {
    let plain = message.encode(1).unwrap();
    let mut wire = vec![0_u8; plain.len() + 16];
    let length = transport.write_message(&plain, &mut wire).unwrap();
    encode_noise_frame(&wire[..length]).unwrap()
}

pub fn send(
    peer: &mut TcpStream,
    bytes: &[u8],
    slow: bool,
    stopped: &AtomicBool,
    progress: &AtomicUsize,
) {
    for byte in bytes {
        if stopped.load(Ordering::Acquire) || peer.write_all(&[*byte]).is_err() {
            return;
        }
        progress.fetch_add(1, Ordering::Release);
        if slow {
            thread::sleep(Duration::from_millis(60));
        }
    }
}

pub fn read_frame(peer: &mut TcpStream) -> Vec<u8> {
    let mut prefix = [0_u8; 2];
    peer.read_exact(&mut prefix).unwrap();
    let mut frame = vec![0_u8; usize::from(u16::from_be_bytes(prefix))];
    peer.read_exact(&mut frame).unwrap();
    frame
}

pub fn check(
    stream: TcpStream,
    mut peer: TcpStream,
    operation: impl FnOnce(TcpStream, Instant) -> bool + Send + 'static,
    peer_operation: impl FnOnce(&mut TcpStream, &AtomicBool, &AtomicUsize) + Send + 'static,
) {
    let shutdown = stream.try_clone().unwrap();
    let peer_shutdown = peer.try_clone().unwrap();
    let stopped = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicUsize::new(0));
    let stop_writer = Arc::clone(&stopped);
    let writer_progress = Arc::clone(&progress);
    let (result_tx, result_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(200);
        let result = operation(stream, deadline);
        let _ = result_tx.send(result);
    });
    let writer = thread::spawn(move || peer_operation(&mut peer, &stop_writer, &writer_progress));
    // Allow ample scheduler slack around the small injected deadline, but
    // finish well before a valid trickled record would have completed.
    let result = result_rx.recv_timeout(Duration::from_millis(800));
    stopped.store(true, Ordering::Release);
    let _ = shutdown.shutdown(Shutdown::Both);
    let _ = peer_shutdown.shutdown(Shutdown::Both);
    reader.join().unwrap();
    writer.join().unwrap();
    assert!(
        progress.load(Ordering::Acquire) >= 3,
        "fixture must supply prefix and payload progress"
    );
    assert!(
        matches!(result, Ok(true)),
        "absolute deadline was exceeded despite partial network progress: {result:?}"
    );
}
