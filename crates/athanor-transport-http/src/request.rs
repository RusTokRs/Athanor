//! Bounded HTTP/1.x request parsing for the read-only documentation server.
//!
//! The server accepts one request per connection and never reads a body. Everything a client can
//! influence is bounded here before any handler runs: the request head, the target, the number of
//! query parameters, and every decoded path segment.

use std::fmt;
use std::net::{IpAddr, Ipv6Addr};

/// Largest accepted request head, including the terminating blank line.
pub(super) const MAX_REQUEST_HEAD_BYTES: usize = 8 * 1024;
/// Largest accepted request target, before percent decoding.
pub(super) const MAX_REQUEST_TARGET_BYTES: usize = 2 * 1024;
/// Longest accepted decoded path segment.
const MAX_SEGMENT_BYTES: usize = 128;
/// Largest accepted number of request headers.
const MAX_HEADERS: usize = 64;
/// Largest accepted number of query parameters.
const MAX_QUERY_PARAMETERS: usize = 16;
/// Largest accepted query parameter name or value, after decoding.
const MAX_QUERY_VALUE_BYTES: usize = 1024;

/// HTTP method the documentation server answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Method {
    Get,
    Head,
}

/// One parsed request head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Request {
    pub(super) method: Method,
    pub(super) target: String,
    /// Value of the `Host` header, when the client sent one.
    pub(super) host: Option<String>,
}

/// Why a request head was rejected before routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestRejection {
    /// The head is not a well-formed HTTP/1.x request.
    Malformed,
    /// The method is understood but not supported.
    MethodNotAllowed,
    /// The request names an HTTP version this server does not speak.
    UnsupportedVersion,
    /// The head exceeds the accepted size.
    Oversized,
    /// An HTTP/1.1 request without `Host`.
    MissingHost,
    /// A host name that does not name the loopback interface.
    UnexpectedHost,
    /// A decoded path segment that escapes the served layout.
    RejectedPath,
    /// A query string that is malformed or exceeds the accepted bounds.
    RejectedQuery,
    /// A target above the fixed URI byte limit.
    TargetTooLong,
}

impl RequestRejection {
    pub(super) fn status(self) -> u16 {
        match self {
            RequestRejection::Malformed
            | RequestRejection::MissingHost
            | RequestRejection::RejectedPath => 400,
            RequestRejection::MethodNotAllowed => 405,
            RequestRejection::UnsupportedVersion => 505,
            RequestRejection::Oversized => 431,
            RequestRejection::UnexpectedHost => 421,
            RequestRejection::RejectedQuery | RequestRejection::TargetTooLong => 414,
        }
    }

    pub(super) fn code(self) -> &'static str {
        match self {
            RequestRejection::Malformed => "malformed_request",
            RequestRejection::MethodNotAllowed => "method_not_allowed",
            RequestRejection::UnsupportedVersion => "unsupported_version",
            RequestRejection::Oversized => "request_too_large",
            RequestRejection::MissingHost => "missing_host",
            RequestRejection::UnexpectedHost => "unexpected_host",
            RequestRejection::RejectedPath => "rejected_path",
            RequestRejection::RejectedQuery => "rejected_query",
            RequestRejection::TargetTooLong => "target_too_long",
        }
    }

    pub(super) fn message(self) -> &'static str {
        match self {
            RequestRejection::Malformed => "the request head is not a well-formed HTTP/1.x request",
            RequestRejection::MethodNotAllowed => {
                "the documentation server answers GET and HEAD only"
            }
            RequestRejection::UnsupportedVersion => {
                "the documentation server speaks HTTP/1.0 and HTTP/1.1"
            }
            RequestRejection::Oversized => "the request head exceeds the accepted size",
            RequestRejection::MissingHost => "an HTTP/1.1 request must carry a Host header",
            RequestRejection::UnexpectedHost => {
                "the Host header must name the loopback interface this server is bound to"
            }
            RequestRejection::RejectedPath => {
                "the request path escapes the served documentation layout"
            }
            RequestRejection::RejectedQuery => {
                "the query string is malformed or exceeds the accepted size"
            }
            RequestRejection::TargetTooLong => "the request target exceeds the accepted size",
        }
    }
}

impl fmt::Display for RequestRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// Parses one request head that already excludes the terminating blank line.
pub(super) fn parse(head: &str) -> Result<Request, RequestRejection> {
    if head.len() > MAX_REQUEST_HEAD_BYTES {
        return Err(RequestRejection::Oversized);
    }
    let mut lines = head
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    let request_line = lines.next().ok_or(RequestRejection::Malformed)?;
    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or(RequestRejection::Malformed)?;
    let target = parts.next().ok_or(RequestRejection::Malformed)?;
    let version = parts.next().ok_or(RequestRejection::Malformed)?;
    if parts.next().is_some() {
        return Err(RequestRejection::Malformed);
    }
    if !is_header_token(method) {
        return Err(RequestRejection::Malformed);
    }
    let method = match method {
        "GET" => Method::Get,
        "HEAD" => Method::Head,
        _ => return Err(RequestRejection::MethodNotAllowed),
    };
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(RequestRejection::UnsupportedVersion);
    }
    if !target.starts_with('/') {
        return Err(RequestRejection::Malformed);
    }
    if target.len() > MAX_REQUEST_TARGET_BYTES {
        return Err(RequestRejection::TargetTooLong);
    }

    let mut host = None;
    let mut content_length_seen = false;
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            break;
        }
        if index >= MAX_HEADERS {
            return Err(RequestRejection::Oversized);
        }
        let (name, value) = line.split_once(':').ok_or(RequestRejection::Malformed)?;
        if !is_header_token(name)
            || value
                .bytes()
                .any(|byte| (byte < 32 && byte != b'\t') || byte == 127)
        {
            return Err(RequestRejection::Malformed);
        }
        if name.eq_ignore_ascii_case("host") {
            let value = value.trim();
            if value.is_empty() || host.is_some() {
                return Err(RequestRejection::Malformed);
            }
            host = Some(value.to_string());
        }
        // No body or transfer coding is consumed, even for GET. Closing after one request must
        // not turn a malformed body declaration into an accepted request.
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(RequestRejection::Malformed);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length_seen || value.trim() != "0" {
                return Err(RequestRejection::Malformed);
            }
            content_length_seen = true;
        }
    }
    if version == "HTTP/1.1" && host.is_none() {
        return Err(RequestRejection::MissingHost);
    }
    Ok(Request {
        method,
        target: target.to_string(),
        host,
    })
}

fn is_header_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Accepted request target: decoded segments plus decoded query parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Target {
    pub(super) segments: Vec<String>,
    pub(super) query: Query,
}

/// Ordered decoded query parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Query {
    pairs: Vec<(String, String)>,
}

impl Query {
    pub(super) fn get(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Splits and decodes a request target without ever leaving the served layout.
pub(super) fn parse_target(target: &str) -> Result<Target, RequestRejection> {
    if target.len() > MAX_REQUEST_TARGET_BYTES {
        return Err(RequestRejection::TargetTooLong);
    }
    if !target.starts_with('/') || target.contains('#') {
        return Err(RequestRejection::RejectedPath);
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };

    let mut segments = Vec::new();
    for raw in path.split('/').skip(1) {
        if raw.is_empty() {
            continue;
        }
        let segment = percent_decode(raw, false).ok_or(RequestRejection::RejectedPath)?;
        if !segment_is_portable(&segment) {
            return Err(RequestRejection::RejectedPath);
        }
        segments.push(segment);
    }
    // A trailing slash is a directory hint, not a segment.
    if segments.len() > 8 {
        return Err(RequestRejection::RejectedPath);
    }

    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(query) = query {
        for raw in query.split('&') {
            if raw.is_empty() {
                continue;
            }
            if pairs.len() >= MAX_QUERY_PARAMETERS {
                return Err(RequestRejection::RejectedQuery);
            }
            let (name, value) = match raw.split_once('=') {
                Some((name, value)) => (name, value),
                None => (raw, ""),
            };
            let name = percent_decode(name, true).ok_or(RequestRejection::RejectedQuery)?;
            let value = percent_decode(value, true).ok_or(RequestRejection::RejectedQuery)?;
            if name.len() > MAX_QUERY_VALUE_BYTES
                || value.len() > MAX_QUERY_VALUE_BYTES
                || name.chars().any(char::is_control)
                || value.chars().any(char::is_control)
                || pairs.iter().any(|(existing, _)| existing == &name)
            {
                return Err(RequestRejection::RejectedQuery);
            }
            pairs.push((name, value));
        }
    }
    Ok(Target {
        segments,
        query: Query { pairs },
    })
}

/// One decoded path segment must be a single portable path component.
fn segment_is_portable(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= MAX_SEGMENT_BYTES
        && segment != "."
        && segment != ".."
        && !segment.contains('/')
        && !segment.contains('\\')
        && !segment.contains('\0')
        && !segment.chars().any(char::is_control)
}

/// True when a `Host` header names the loopback interface.
pub(super) fn host_is_loopback(host: &str) -> bool {
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        let Some((address, suffix)) = rest.split_once(']') else {
            return false;
        };
        if address.parse::<Ipv6Addr>().is_err() {
            return false;
        }
        let port = match suffix {
            "" => None,
            _ => match suffix.strip_prefix(':') {
                Some(port) => Some(port),
                None => return false,
            },
        };
        (address, port)
    } else {
        match host.split_once(':') {
            Some((name, port)) => (name, Some(port)),
            None => (host, None),
        }
    };
    if port.is_some_and(|port| {
        port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().is_err()
    }) {
        return false;
    }
    name.eq_ignore_ascii_case("localhost")
        || name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Decodes `%XX` escapes; `plus_as_space` additionally turns `+` into a space.
pub(super) fn percent_decode(input: &str, plus_as_space: bool) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let high = hex_value(*bytes.get(index + 1)?)?;
                let low = hex_value(*bytes.get(index + 2)?)?;
                decoded.push(high * 16 + low);
                index += 3;
            }
            b'+' if plus_as_space => {
                decoded.push(b' ');
                index += 1;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_headers_and_request_bodies() {
        for headers in [
            "Host: localhost\r\nHost: evil.example",
            "Host: localhost\r\nHost: localhost",
            "Host : localhost",
            "Host: localhost\r\nBad Header: x",
            "Host: localhost\r\nX: a\0b",
            "Host: localhost\r\nContent-Length: 1",
            "Host: localhost\r\nContent-Length: 0\r\nContent-Length: 0",
            "Host: localhost\r\nTransfer-Encoding: chunked",
        ] {
            assert_eq!(
                parse(&format!("GET / HTTP/1.1\r\n{headers}")),
                Err(RequestRejection::Malformed)
            );
        }
    }

    #[test]
    fn rejects_duplicate_parameters_controls_and_oversized_targets() {
        for target in ["/search?q=x&q=y", "/search?q=%00", "/search?q=%0A"] {
            assert_eq!(parse_target(target), Err(RequestRejection::RejectedQuery));
        }
        let oversized = format!("/{}", "x".repeat(MAX_REQUEST_TARGET_BYTES));
        assert_eq!(
            parse_target(&oversized),
            Err(RequestRejection::TargetTooLong)
        );
        assert_eq!(
            parse_target("/x#fragment"),
            Err(RequestRejection::RejectedPath)
        );
    }

    #[test]
    fn parses_a_bounded_get_request() {
        let request = parse("GET /healthz HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nAccept: */*")
            .expect("valid request");
        assert_eq!(request.method, Method::Get);
        assert_eq!(request.target, "/healthz");
        assert_eq!(request.host.as_deref(), Some("127.0.0.1:8080"));
    }

    #[test]
    fn rejects_mutating_methods_before_routing() {
        for method in ["POST", "PUT", "DELETE", "PATCH", "OPTIONS"] {
            let head = format!("{method} / HTTP/1.1\r\nHost: 127.0.0.1\r\n");
            assert_eq!(parse(&head), Err(RequestRejection::MethodNotAllowed));
        }
    }

    #[test]
    fn rejects_http_11_without_host() {
        assert_eq!(
            parse("GET / HTTP/1.1\r\nAccept: */*"),
            Err(RequestRejection::MissingHost)
        );
    }

    #[test]
    fn rejects_unknown_versions_and_targets() {
        assert_eq!(
            parse("GET / HTTP/2.0\r\nHost: 127.0.0.1"),
            Err(RequestRejection::UnsupportedVersion)
        );
        assert_eq!(
            parse("GET * HTTP/1.1\r\nHost: 127.0.0.1"),
            Err(RequestRejection::Malformed)
        );
        assert_eq!(
            parse("GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nbroken-header"),
            Err(RequestRejection::Malformed)
        );
    }

    #[test]
    fn accepts_only_loopback_hosts() {
        for host in [
            "127.0.0.1",
            "127.0.0.1:8080",
            "localhost",
            "localhost:3000",
            "LOCALHOST:3000",
            "127.0.0.2:3000",
            "[::1]",
            "[::1]:8080",
        ] {
            assert!(host_is_loopback(host), "{host} must be accepted");
        }
        for host in [
            "example.com",
            "example.com:8080",
            "127.0.0.1.evil.test",
            "localhost.evil.test:80",
            "[::1].evil.test",
            "[::1]:invalid",
            "[::1]:65536",
            "[::1]:",
            "127.0.0.1:65536",
            "localhost:",
            "::1",
            "10.0.0.1",
        ] {
            assert!(!host_is_loopback(host), "{host} must be rejected");
        }
    }

    #[test]
    fn splits_and_decodes_the_target() {
        let target =
            parse_target("/g/gen-1/raw/architecture/index.md?a=b%20c&d=1+2").expect("target");
        assert_eq!(
            target.segments,
            vec!["g", "gen-1", "raw", "architecture", "index.md"]
        );
        assert_eq!(target.query.get("a"), Some("b c"));
        assert_eq!(target.query.get("d"), Some("1 2"));
        assert_eq!(target.query.get("missing"), None);
    }

    #[test]
    fn rejects_escaped_traversal_segments() {
        for target in [
            "/../etc/passwd",
            "/g/%2e%2e/manifest.json",
            "/g/gen%2F1",
            "/g/gen-1/raw/..%5cwin.ini",
            "/g/gen-1/raw/%00",
        ] {
            assert_eq!(parse_target(target), Err(RequestRejection::RejectedPath));
        }
    }

    #[test]
    fn rejects_oversized_queries() {
        let long = "a".repeat(MAX_QUERY_VALUE_BYTES + 1);
        assert_eq!(
            parse_target(&format!("/search?q={long}")),
            Err(RequestRejection::RejectedQuery)
        );
        let many = (0..=MAX_QUERY_PARAMETERS)
            .map(|index| format!("k{index}=v"))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(
            parse_target(&format!("/search?{many}")),
            Err(RequestRejection::RejectedQuery)
        );
    }

    #[test]
    fn decodes_percent_escapes_without_losing_utf8() {
        assert_eq!(percent_decode("%D0%B0", false).as_deref(), Some("а"));
        assert_eq!(percent_decode("a+b", true).as_deref(), Some("a b"));
        assert_eq!(percent_decode("%zz", false), None);
        assert_eq!(percent_decode("%4", false), None);
    }

    #[test]
    fn treats_a_trailing_slash_as_a_directory_hint() {
        assert_eq!(
            parse_target("/g/gen-1/").map(|t| t.segments),
            Ok(vec!["g".to_string(), "gen-1".to_string()])
        );
    }
}
