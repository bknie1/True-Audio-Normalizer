//! TAN as a Stremio addon: serves your own local video library back into
//! Stremio's stream picker with a TAN-normalized audio track alongside the
//! original, the same "extra Play Version" idea as the Jellyfin plugin.
//!
//! Stremio addons only ever get to *offer a stream URL* before playback
//! starts - there's no hook into a player's live audio pipeline, so an
//! addon can't touch audio the way tan-live/tan-tray do for real-time
//! desktop capture. What it *can* do is exactly what this does: pre-process
//! your own files once and hand back an alternate, already-normalized
//! stream for Stremio to play like any other.
//!
//! Hand-rolled HTTP/1.1 over `std::net::TcpListener`, no web framework, no
//! JSON crate - consistent with the rest of the project (hand-written WAV
//! codec, hand-written ring buffer, tan-server's own hand-rolled HTTP). Video
//! demux/mux is shelled out to `ffmpeg`, the same approach the Jellyfin
//! plugin uses via its host's bundled copy - this addon expects `ffmpeg` on
//! PATH (or pointed at via `--ffmpeg`).

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tan_core::{normalize_offline, wav, Profile};

struct Config {
    port: u16,
    bind: String,
    library: PathBuf,
    output: PathBuf,
    profile_name: String,
    ffmpeg: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }

    let Some(library) = flag_value(&args, "--library").map(PathBuf::from) else {
        eprintln!("--library <dir> is required\n");
        print_usage();
        std::process::exit(1);
    };
    if !library.is_dir() {
        eprintln!("--library {} is not a directory", library.display());
        std::process::exit(1);
    }

    let output = flag_value(&args, "--output").map(PathBuf::from).unwrap_or_else(|| library.join(".tan-cache"));
    let cfg = Config {
        port: flag_value(&args, "--port").and_then(|s| s.parse().ok()).unwrap_or(5860),
        bind: flag_value(&args, "--bind").unwrap_or_else(|| "127.0.0.1".to_string()),
        library,
        output,
        profile_name: flag_value(&args, "--profile").unwrap_or_else(|| "movie".to_string()),
        ffmpeg: flag_value(&args, "--ffmpeg").unwrap_or_else(|| "ffmpeg".to_string()),
    };
    let Some(profile) = profile_by_name(&cfg.profile_name) else {
        eprintln!("unknown profile '{}' (expected: universal, movie, music, speech, night, game)", cfg.profile_name);
        std::process::exit(1);
    };

    if let Err(e) = std::fs::create_dir_all(&cfg.output) {
        eprintln!("couldn't create output dir {}: {e}", cfg.output.display());
        std::process::exit(1);
    }

    println!("TAN Stremio addon: scanning {}", cfg.library.display());
    let mut sources = Vec::new();
    scan_videos(&cfg.library, &cfg.output, &mut sources);
    println!("  found {} video file(s)", sources.len());

    let entries: Vec<LibraryEntry> = sources.into_iter().map(|src| LibraryEntry::new(src, &cfg.output)).collect();

    println!("Normalizing anything not already cached (profile: {})...", cfg.profile_name);
    for entry in &entries {
        if entry.cache_path.exists() {
            continue;
        }
        print!("  {} ... ", entry.name);
        io::stdout().flush().ok();
        match normalize_entry(entry, profile, &cfg.ffmpeg) {
            Ok(()) => println!("done"),
            Err(e) => println!("FAILED: {e}"),
        }
    }

    let addr = format!("{}:{}", cfg.bind, cfg.port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("couldn't bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("\nTAN Stremio addon listening on http://{addr}");
    println!("Install in Stremio with manifest URL: http://{addr}/manifest.json");
    if cfg.bind != "127.0.0.1" && cfg.bind != "localhost" {
        eprintln!("warning: bound to {}, not just localhost - this addon has no authentication.", cfg.bind);
    }

    let entries = Arc::new(entries);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let entries = Arc::clone(&entries);
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(&stream, &entries, cfg.port) {
                        eprintln!("connection error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

fn print_usage() {
    println!(
        "usage: tan-stremio --library <dir> [--port <n>] [--bind <address>] \
         [--output <dir>] [--profile <name>] [--ffmpeg <path>]\n\n\
         Scans --library (recursively) for video files, normalizes each one's\n\
         audio into --output (default: <library>/.tan-cache) the first time\n\
         it's seen, then serves a Stremio addon offering each as a \"TAN\n\
         Normalized\" stream alongside whatever else provides the original.\n\n\
         Requires ffmpeg on PATH (or pass --ffmpeg <path> to it) to demux and\n\
         remux video containers; profile defaults to \"movie\" (also:\n\
         universal, music, speech, night, game). Re-run to pick up new files -\n\
         the library is scanned once at startup."
    );
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
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

// --- Library scanning -------------------------------------------------

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm", "m4v"];

struct LibraryEntry {
    id: String,
    name: String,
    source_path: PathBuf,
    cache_path: PathBuf,
}

impl LibraryEntry {
    fn new(source_path: PathBuf, output_dir: &Path) -> Self {
        let id = stable_id(&source_path);
        let name = source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let cache_path = output_dir.join(format!("{id}.mkv"));
        Self { id, name, source_path, cache_path }
    }
}

/// FNV-1a 64-bit over the absolute path - stable across process restarts
/// (unlike `DefaultHasher`, which is randomly seeded per-process), and
/// dependency-free.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_id(path: &Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("{:016x}", fnv1a64(abs.to_string_lossy().as_bytes()))
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn scan_videos(dir: &Path, output_dir: &Path, out: &mut Vec<PathBuf>) {
    let output_dir = std::fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf());
    scan_videos_inner(dir, &output_dir, out);
}

fn scan_videos_inner(dir: &Path, output_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if canon == *output_dir {
            continue; // never treat our own cache output as source material
        }
        if path.is_dir() {
            scan_videos_inner(&path, output_dir, out);
        } else if is_video(&path) {
            out.push(path);
        }
    }
}

// --- Normalization pipeline (ffmpeg demux -> tan-core -> ffmpeg remux) -

fn normalize_entry(entry: &LibraryEntry, profile: Profile, ffmpeg: &str) -> Result<(), String> {
    let tmp_dir = std::env::temp_dir();
    let extracted = tmp_dir.join(format!("tan-stremio-extract-{}.wav", entry.id));
    let normalized = tmp_dir.join(format!("tan-stremio-normalized-{}.wav", entry.id));
    let result = (|| {
        extract_wav(ffmpeg, &entry.source_path, &extracted)?;
        normalize_wav_file(&extracted, &normalized, profile)?;
        let tmp_out = entry.cache_path.with_extension("mkv.tmp");
        mux(ffmpeg, &entry.source_path, &normalized, &tmp_out)?;
        std::fs::rename(&tmp_out, &entry.cache_path).map_err(|e| format!("couldn't finalize output: {e}"))
    })();
    std::fs::remove_file(&extracted).ok();
    std::fs::remove_file(&normalized).ok();
    result
}

fn extract_wav(ffmpeg: &str, source: &Path, wav_out: &Path) -> Result<(), String> {
    run_ffmpeg(ffmpeg, &["-y", "-i", &path_str(source), "-vn", "-acodec", "pcm_s16le", "-ar", "48000", &path_str(wav_out)])
}

fn mux(ffmpeg: &str, source_video: &Path, normalized_wav: &Path, out: &Path) -> Result<(), String> {
    run_ffmpeg(
        ffmpeg,
        &[
            "-y",
            "-i", &path_str(source_video),
            "-i", &path_str(normalized_wav),
            "-map", "0:v:0",
            "-map", "1:a:0",
            "-c:v", "copy",
            "-c:a", "aac",
            "-b:a", "192k",
            &path_str(out),
        ],
    )
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn run_ffmpeg(ffmpeg: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(ffmpeg)
        .args(args)
        .output()
        .map_err(|e| format!("couldn't run '{ffmpeg}': {e}"))?;
    if !output.status.success() {
        return Err(format!("ffmpeg exited {}: {}", output.status, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn normalize_wav_file(in_path: &Path, out_path: &Path, profile: Profile) -> Result<(), String> {
    let (spec, mut samples) = wav::read_wav(&path_str(in_path)).map_err(|e| format!("couldn't read extracted audio: {e}"))?;
    normalize_offline(&mut samples, spec.sample_rate, spec.channels as usize, profile);
    wav::write_wav(&path_str(out_path), &spec, &samples).map_err(|e| format!("couldn't write normalized audio: {e}"))
}

// --- Stremio addon HTTP ------------------------------------------------

struct Request {
    method: String,
    path: String,
}

fn read_request<R: BufRead>(r: &mut R) -> io::Result<Request> {
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "connection closed before any request line"));
    }
    let mut parts = line.trim_end().split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    // Drain headers; this addon doesn't need any of them, but must consume
    // them so the connection is left in a clean state.
    loop {
        let mut hline = String::new();
        if r.read_line(&mut hline)? == 0 || hline.trim_end().is_empty() {
            break;
        }
    }
    Ok(Request { method, path })
}

fn write_headers<W: Write>(w: &mut W, status: u16, content_type: &str, content_length: usize) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };
    write!(w, "HTTP/1.1 {status} {reason}\r\n")?;
    write!(w, "Content-Type: {content_type}\r\n")?;
    write!(w, "Content-Length: {content_length}\r\n")?;
    // Stremio's client fetches addon resources with CORS enforced; a local
    // addon has no meaningful "origin" of its own to restrict this to.
    write!(w, "Access-Control-Allow-Origin: *\r\n")?;
    write!(w, "Connection: close\r\n\r\n")
}

fn write_json<W: Write>(w: &mut W, status: u16, body: &str) -> io::Result<()> {
    write_headers(w, status, "application/json", body.len())?;
    w.write_all(body.as_bytes())
}

fn write_text<W: Write>(w: &mut W, status: u16, body: &str) -> io::Result<()> {
    write_headers(w, status, "text/plain", body.len())?;
    w.write_all(body.as_bytes())
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Decode `%XX` escapes in a URL path segment - Stremio percent-encodes the
/// `:` in ids like `tan:<hash>` when building the request path.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn manifest_json() -> String {
    format!(
        r#"{{"id":"com.tan.stremio","version":"{}","name":"TAN Normalizer","description":"Serves your own local video library with a TAN-normalized audio track alongside the original - no proprietary metadata, everything processed on your own machine.","resources":["catalog","stream"],"types":["movie"],"catalogs":[{{"type":"movie","id":"tan-local","name":"TAN Local Library"}}],"idPrefixes":["tan:"]}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn catalog_json(entries: &[LibraryEntry]) -> String {
    let metas: Vec<String> = entries
        .iter()
        .map(|e| format!(r#"{{"id":"tan:{}","type":"movie","name":"{}"}}"#, e.id, json_escape(&e.name)))
        .collect();
    format!(r#"{{"metas":[{}]}}"#, metas.join(","))
}

fn stream_json(entry: &LibraryEntry, port: u16) -> String {
    format!(
        r#"{{"streams":[{{"title":"TAN Normalized","name":"TAN","url":"http://127.0.0.1:{port}/file/{}.mkv"}}]}}"#,
        entry.id
    )
}

fn handle_connection(stream: &TcpStream, entries: &[LibraryEntry], port: u16) -> io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream);
    let req = match read_request(&mut reader) {
        Ok(r) => r,
        Err(_) => return Ok(()), // malformed/empty request - nothing to usefully respond to
    };
    let mut w = stream;

    if req.method == "OPTIONS" {
        return write_headers(&mut w, 200, "text/plain", 0);
    }
    if req.method != "GET" && req.method != "HEAD" {
        return write_text(&mut w, 404, "not found");
    }

    let path = req.path.as_str();
    if path == "/" || path == "/manifest.json" {
        return write_json(&mut w, 200, &manifest_json());
    }
    if path == "/catalog/movie/tan-local.json" {
        return write_json(&mut w, 200, &catalog_json(entries));
    }
    if let Some(rest) = path.strip_prefix("/stream/movie/").and_then(|s| s.strip_suffix(".json")) {
        let id = percent_decode(rest);
        let Some(hash) = id.strip_prefix("tan:") else {
            return write_json(&mut w, 200, r#"{"streams":[]}"#);
        };
        return match entries.iter().find(|e| e.id == hash) {
            Some(entry) if entry.cache_path.exists() => write_json(&mut w, 200, &stream_json(entry, port)),
            _ => write_json(&mut w, 200, r#"{"streams":[]}"#),
        };
    }
    if let Some(rest) = path.strip_prefix("/file/").and_then(|s| s.strip_suffix(".mkv")) {
        return match entries.iter().find(|e| e.id == rest) {
            Some(entry) if entry.cache_path.exists() => serve_file(&mut w, &entry.cache_path),
            _ => write_text(&mut w, 404, "not found"),
        };
    }

    write_text(&mut w, 404, "not found")
}

fn serve_file<W: Write>(w: &mut W, path: &Path) -> io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len() as usize;
    write_headers(w, 200, "video/x-matroska", len)?;
    io::copy(&mut file, w)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn stable_id_is_deterministic_and_path_specific() {
        let a = fnv1a64(b"/movies/one.mkv");
        let b = fnv1a64(b"/movies/one.mkv");
        let c = fnv1a64(b"/movies/two.mkv");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn is_video_matches_known_extensions_case_insensitively() {
        assert!(is_video(Path::new("Movie.MKV")));
        assert!(is_video(Path::new("clip.mp4")));
        assert!(!is_video(Path::new("readme.txt")));
        assert!(!is_video(Path::new("noext")));
    }

    #[test]
    fn percent_decode_handles_the_colon_stremio_actually_sends() {
        assert_eq!(percent_decode("tan%3Aabc123"), "tan:abc123");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape(r#"a "quoted" name"#), r#"a \"quoted\" name"#);
        assert_eq!(json_escape(r"back\slash"), r"back\\slash");
    }

    #[test]
    fn manifest_advertises_the_catalog_and_stream_resources() {
        let m = manifest_json();
        assert!(m.contains("\"resources\":[\"catalog\",\"stream\"]"));
        assert!(m.contains("\"idPrefixes\":[\"tan:\"]"));
    }

    #[test]
    fn catalog_lists_every_entry_by_prefixed_id() {
        let entries = vec![
            LibraryEntry { id: "abc".into(), name: "Movie One".into(), source_path: PathBuf::new(), cache_path: PathBuf::new() },
        ];
        let json = catalog_json(&entries);
        assert!(json.contains(r#""id":"tan:abc""#));
        assert!(json.contains(r#""name":"Movie One""#));
    }

    fn spawn_test_server(entries: Vec<LibraryEntry>) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let entries = Arc::new(entries);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let entries = Arc::clone(&entries);
                    let _ = handle_connection(&stream, &entries, addr.port());
                }
            }
        });
        addr
    }

    fn http_get(addr: std::net::SocketAddr, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        let text = String::from_utf8_lossy(&resp).into_owned();
        let sep = text.find("\r\n\r\n").expect("no header/body separator");
        let status: u16 = text[9..12].parse().unwrap();
        (status, text[sep + 4..].to_string())
    }

    #[test]
    fn serves_manifest_over_http() {
        let addr = spawn_test_server(Vec::new());
        let (status, body) = http_get(addr, "/manifest.json");
        assert_eq!(status, 200);
        assert!(body.contains("\"id\":\"com.tan.stremio\""));
    }

    #[test]
    fn stream_lookup_for_unknown_id_returns_empty_streams_not_an_error() {
        let addr = spawn_test_server(Vec::new());
        let (status, body) = http_get(addr, "/stream/movie/tan%3Adoesnotexist.json");
        assert_eq!(status, 200);
        assert_eq!(body.trim(), r#"{"streams":[]}"#);
    }

    #[test]
    fn unknown_path_is_404() {
        let addr = spawn_test_server(Vec::new());
        let (status, _) = http_get(addr, "/nope");
        assert_eq!(status, 404);
    }
}
