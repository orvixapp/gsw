use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const MAX_RESPONSE_BYTES: u64 = 128 * 1024;
const IO_TIMEOUT: Duration = Duration::from_millis(750);

pub fn read_goroutine_count(base_url: &str) -> Result<u64, String> {
    let endpoint = HttpEndpoint::parse(base_url)?;
    let address = endpoint
        .authority
        .to_socket_addrs()
        .map_err(|err| format!("failed to resolve pprof endpoint: {err}"))?
        .next()
        .ok_or("pprof endpoint resolved to no addresses")?;
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)
        .map_err(|err| format!("failed to connect to pprof: {err}"))?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: text/plain\r\n\r\n",
        endpoint.goroutine_path, endpoint.authority
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("failed to request pprof: {err}"))?;
    let mut response = String::new();
    stream
        .take(MAX_RESPONSE_BYTES)
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read pprof response: {err}"))?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or("invalid HTTP response from pprof")?;
    if !headers.lines().next().unwrap_or_default().contains(" 200 ") {
        return Err(format!(
            "pprof returned {}",
            headers.lines().next().unwrap_or("an invalid status")
        ));
    }
    parse_goroutine_count(body).ok_or("pprof response did not contain a goroutine count".into())
}

fn parse_goroutine_count(body: &str) -> Option<u64> {
    let marker = "goroutine profile: total ";
    let rest = body.split(marker).nth(1)?;
    rest.split_whitespace().next()?.parse().ok()
}

struct HttpEndpoint {
    authority: String,
    goroutine_path: String,
}

impl HttpEndpoint {
    fn parse(base_url: &str) -> Result<Self, String> {
        let value = base_url
            .strip_prefix("http://")
            .ok_or("pprof URL must start with http://")?;
        let (authority, base_path) = value.split_once('/').unwrap_or((value, ""));
        if authority.is_empty() {
            return Err("pprof URL is missing a host".to_string());
        }
        let authority = if authority.contains(':') {
            authority.to_string()
        } else {
            format!("{authority}:80")
        };
        let base_path = base_path.trim_matches('/');
        let goroutine_path = if base_path.is_empty() {
            "/debug/pprof/goroutine?debug=1".to_string()
        } else {
            format!("/{base_path}/debug/pprof/goroutine?debug=1")
        };
        Ok(Self {
            authority,
            goroutine_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plaintext_goroutine_profile() {
        assert_eq!(
            parse_goroutine_count("goroutine profile: total 143\n1 @ 0x123"),
            Some(143)
        );
    }

    #[test]
    fn builds_default_pprof_path() {
        let endpoint = HttpEndpoint::parse("http://127.0.0.1:6060").unwrap();
        assert_eq!(endpoint.authority, "127.0.0.1:6060");
        assert_eq!(endpoint.goroutine_path, "/debug/pprof/goroutine?debug=1");
    }
}
