//! Trickle traffic cannot restart the server's absolute I/O deadline.

mod fixture {
    use std::{
        io::Write,
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
        let mut initiator =
            HandshakeSession::trusted_initiator(&client, server.public_key()).unwrap();
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
        let writer =
            thread::spawn(move || peer_operation(&mut peer, &stop_writer, &writer_progress));
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
}

use super::*;

#[test]
fn server_handshake_deadline_survives_partial_frame_progress() {
    let client = NoiseKeypair::generate().unwrap();
    let server = NoiseKeypair::generate().unwrap();
    let mut handshake = HandshakeSession::trusted_initiator(&client, server.public_key()).unwrap();
    let mut buffer = [0_u8; 512];
    let count = handshake.write_handshake(&[], &mut buffer).unwrap();
    let frame = crate::framing::encode_noise_frame(&buffer[..count]).unwrap();
    let (peer, stream) = fixture::sockets();
    fixture::check(
        stream,
        peer,
        move |mut stream, deadline| {
            establish_transport(&mut stream, &server, SessionMode::Trusted, deadline).is_err()
        },
        move |peer, stopped, progress| fixture::send(peer, &frame, true, stopped, progress),
    );
}

fn receive_case(slow: bool) {
    let (mut client, mut server) = fixture::noise_pair();
    let frame = fixture::record(&mut client, &Message::HeadRequest);
    let (peer, stream) = fixture::sockets();
    fixture::check(
        stream,
        peer,
        move |mut stream, deadline| {
            let mut protocol = ProtocolState::new(SessionMode::Trusted, LocalRole::Server);
            protocol.authenticate();
            let result = receive_message(&mut stream, &mut server, &mut protocol, deadline);
            if slow {
                result.is_err()
            } else {
                matches!(result, Ok((1, Message::HeadRequest)))
            }
        },
        move |peer, stopped, progress| fixture::send(peer, &frame, slow, stopped, progress),
    );
}

#[test]
fn server_authenticated_deadline_survives_partial_frame_progress() {
    receive_case(true);
}

#[test]
fn server_accepts_a_complete_valid_record_before_deadline() {
    receive_case(false);
}
