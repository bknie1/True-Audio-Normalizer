//! TAN as a local web API: POST a WAV file, get back the TAN-processed WAV.
//!
//! This is the shared foundation for platform integrations - a Jellyfin
//! plugin, a future Stremio-facing proxy, a simple script, anything that can
//! make an HTTP request - so the actual DSP integration gets built once,
//! here, instead of re-embedded per platform.
//!
//! Hand-rolled HTTP/1.1 over `std::net::TcpListener`, no web framework and no
//! async runtime - consistent with the rest of the project (hand-written WAV
//! codec, hand-written ring buffer): one request per connection, a
//! `Content-Length` body, no chunked transfer-encoding, no keep-alive. That
//! is a deliberate, honest scope - this is a small local processing endpoint,
//! not a general-purpose web server, and it has no authentication, which is
//! why it binds to localhost only unless told otherwise.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use tan_core::{normalize_offline, wav, Profile};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }
    let port: u16 = flag_value(&args, "--port").and_then(|s| s.parse().ok()).unwrap_or(5859);
    let bind = flag_value(&args, "--bind").unwrap_or_else(|| "127.0.0.1".to_string());

    let addr = format!("{bind}:{port}");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("couldn't bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("TAN server listening on http://{addr}");
    println!("  GET  /health");
    println!("  POST /normalize?profile=<universal|movie|music|speech|night|game>  (WAV body in, WAV body out)");
    if bind != "127.0.0.1" && bind != "localhost" {
        eprintln!("warning: bound to {bind}, not just localhost - this API has no authentication.");
    }
    serve_forever(listener);
}

fn print_usage() {
    println!(
        "usage: tan-server [--port <n>] [--bind <address>]\n\n\
         Defaults: --port 5859, --bind 127.0.0.1 (localhost only - this API\n\
         has no authentication, so only bind elsewhere behind something that\n\
         provides it)."
    );
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn serve_forever(listener: TcpListener) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(&stream) {
                        eprintln!("connection error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

struct Request {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
}

/// Look up a `key=value` pair in a raw (already-decoded-enough) query string.
/// Deliberately does not percent-decode - profile names are always plain
/// ASCII identifiers, so there's nothing for a real client to encode.
fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then_some(v)
    })
}

fn bad_request(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn read_request<R: BufRead>(r: &mut R) -> io::Result<Request> {
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Err(bad_request("connection closed before any request line"));
    }
    let mut parts = line.trim_end().split_whitespace();
    let method = parts.next().ok_or_else(|| bad_request("empty request line"))?.to_string();
    let target = parts.next().ok_or_else(|| bad_request("missing request path"))?.to_string();
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };

    let mut headers = Vec::new();
    loop {
        let mut hline = String::new();
        r.read_line(&mut hline)?;
        let hline = hline.trim_end();
        if hline.is_empty() {
            break; // blank line ends the header block
        }
        if let Some((k, v)) = hline.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    let content_length: usize = header_value(&headers, "content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        r.read_exact(&mut body)?;
    }

    Ok(Request { method, path, query, body })
}

fn write_response<W: Write>(w: &mut W, status: u16, content_type: &str, body: &[u8]) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        415 => "Unsupported Media Type",
        _ => "Error",
    };
    write!(w, "HTTP/1.1 {status} {reason}\r\n")?;
    write!(w, "Content-Type: {content_type}\r\n")?;
    write!(w, "Content-Length: {}\r\n", body.len())?;
    write!(w, "Connection: close\r\n\r\n")?;
    w.write_all(body)?;
    w.flush()
}

fn profile_by_name(name: &str) -> Option<Profile> {
    Some(match name {
        "universal" => Profile::universal(),
        "movie" => Profile::movie(),
        "music" => Profile::music(),
        "speech" => Profile::speech(),
        "night" => Profile::night(),
        "game" => Profile::game(),
        _ => return None,
    })
}

fn handle_connection(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream);
    let req = match read_request(&mut reader) {
        Ok(r) => r,
        Err(e) => {
            let mut w = stream;
            return write_response(&mut w, 400, "text/plain", format!("bad request: {e}").as_bytes());
        }
    };
    let mut w = stream;
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/health") => {
            let body = format!("TAN server v{}\n", env!("CARGO_PKG_VERSION"));
            write_response(&mut w, 200, "text/plain", body.as_bytes())
        }
        ("POST", "/normalize") => handle_normalize(&req, &mut w),
        _ => write_response(&mut w, 404, "text/plain", b"not found"),
    }
}

fn handle_normalize<W: Write>(req: &Request, w: &mut W) -> io::Result<()> {
    let profile_name = query_param(&req.query, "profile").unwrap_or("universal");
    let Some(profile) = profile_by_name(profile_name) else {
        let msg = format!(
            "unknown profile '{profile_name}' (expected: universal, movie, music, speech, night, game)"
        );
        return write_response(w, 400, "text/plain", msg.as_bytes());
    };

    let (spec, mut samples) = match wav::read_wav_from(io::Cursor::new(&req.body)) {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("couldn't read the request body as a WAV file: {e}");
            return write_response(w, 415, "text/plain", msg.as_bytes());
        }
    };

    normalize_offline(&mut samples, spec.sample_rate, spec.channels as usize, profile);

    let mut out_body = Vec::new();
    wav::write_wav_to(&mut out_body, &spec, &samples)?;
    write_response(w, 200, "audio/wav", &out_body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn query_param_finds_the_right_pair() {
        assert_eq!(query_param("profile=movie&x=1", "profile"), Some("movie"));
        assert_eq!(query_param("x=1&profile=night", "profile"), Some("night"));
        assert_eq!(query_param("x=1", "profile"), None);
        assert_eq!(query_param("", "profile"), None);
    }

    fn spawn_test_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let _ = handle_connection(&stream);
                }
            }
        });
        addr
    }

    fn wav_body(sample_rate: u32, channels: u16, n: usize) -> Vec<u8> {
        let spec = wav::WavSpec { sample_rate, channels, bits_per_sample: 16 };
        let samples: Vec<f32> = (0..n * channels as usize)
            .map(|i| 0.4 * (i as f32 * 0.05).sin())
            .collect();
        let mut buf = Vec::new();
        wav::write_wav_to(&mut buf, &spec, &samples).unwrap();
        buf
    }

    fn http_post(addr: std::net::SocketAddr, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(stream, "POST {path} HTTP/1.1\r\nContent-Length: {}\r\n\r\n", body.len()).unwrap();
        stream.write_all(body).unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        let sep = resp.windows(4).position(|w| w == b"\r\n\r\n").expect("no header/body separator");
        let status: u16 = std::str::from_utf8(&resp[9..12]).unwrap().parse().unwrap();
        (status, resp[sep + 4..].to_vec())
    }

    #[test]
    fn end_to_end_normalize_round_trip_is_a_valid_wav() {
        let addr = spawn_test_server();
        let body = wav_body(8000, 1, 400);
        let (status, resp_body) = http_post(addr, "/normalize?profile=universal", &body);
        assert_eq!(status, 200);
        let (out_spec, out_samples) = wav::read_wav_from(io::Cursor::new(&resp_body)).unwrap();
        assert_eq!(out_spec.sample_rate, 8000);
        assert_eq!(out_spec.channels, 1);
        assert_eq!(out_samples.len(), 400);
    }

    #[test]
    fn unknown_profile_is_a_400_not_a_crash() {
        let addr = spawn_test_server();
        let body = wav_body(8000, 1, 100);
        let (status, resp_body) = http_post(addr, "/normalize?profile=nonsense", &body);
        assert_eq!(status, 400);
        assert!(String::from_utf8_lossy(&resp_body).contains("unknown profile"));
    }

    #[test]
    fn non_wav_body_is_415_not_a_crash() {
        let addr = spawn_test_server();
        let (status, _) = http_post(addr, "/normalize", b"not a wav file at all");
        assert_eq!(status, 415);
    }
}
