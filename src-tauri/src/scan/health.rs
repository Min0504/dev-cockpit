//! Service health probes: TCP connect + minimal HTTP status check.
//! Zero-dependency (std::net); localhost dev servers only.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TCP_TIMEOUT: Duration = Duration::from_millis(400);
const HTTP_TIMEOUT: Duration = Duration::from_millis(900);

fn connect(port: u16) -> Option<TcpStream> {
    let v4: SocketAddr = ([127, 0, 0, 1], port).into();
    if let Ok(s) = TcpStream::connect_timeout(&v4, TCP_TIMEOUT) {
        return Some(s);
    }
    let v6: SocketAddr = (std::net::Ipv6Addr::LOCALHOST, port).into();
    TcpStream::connect_timeout(&v6, TCP_TIMEOUT).ok()
}

/// Can we open a TCP connection to localhost:port?
pub fn tcp_check(port: u16) -> bool {
    connect(port).is_some()
}

/// Issue `GET /` and return the HTTP status code, if the peer speaks HTTP.
/// Any well-formed response (including 404/500) proves the server is alive.
pub fn http_check(port: u16) -> Option<u16> {
    let mut stream = connect(port)?;
    stream.set_read_timeout(Some(HTTP_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(HTTP_TIMEOUT)).ok()?;
    let req = format!(
        "GET / HTTP/1.1\r\nHost: localhost:{port}\r\nUser-Agent: dev-cockpit\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).ok()?;
    parse_status_line(&buf[..n])
}

pub fn parse_status_line(buf: &[u8]) -> Option<u16> {
    let text = std::str::from_utf8(buf).ok()?;
    let line = text.lines().next()?;
    if !line.starts_with("HTTP/") {
        return None;
    }
    line.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn parses_status_lines() {
        assert_eq!(parse_status_line(b"HTTP/1.1 200 OK\r\n"), Some(200));
        assert_eq!(parse_status_line(b"HTTP/1.0 404 Not Found\r\n"), Some(404));
        assert_eq!(parse_status_line(b"SSH-2.0-OpenSSH"), None);
        assert_eq!(parse_status_line(&[0xff, 0xfe]), None);
    }

    #[test]
    fn tcp_and_http_against_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            // Serve every probe connection the test makes.
            while let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf);
                let _ = sock.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
            }
        });
        assert!(tcp_check(port));
        assert_eq!(http_check(port), Some(204));
        // Nothing listens here (port was just released or never bound):
        assert!(!tcp_check(1)); // port 1 requires root to bind; nothing listens in tests
    }
}
