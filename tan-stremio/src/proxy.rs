//! `tan-stremio proxy` - a real-time TAN Stremio addon for *streamed* content.
//!
//! Stremio addons can't touch a player's audio pipeline, so this doesn't try.
//! Instead it wraps an upstream stream addon (your Torrentio/Comet/etc.,
//! configured with a debrid service so its streams are direct HTTP URLs) and
//! offers, alongside each of those, a "TAN" variant whose URL points back at a
//! local transcode proxy. When the player opens that URL, ffmpeg pulls the
//! original stream, pipes its audio through TAN in real time (see
//! `pcm-filter`), copies the video untouched, and remuxes to a live MPEG-TS
//! stream. Net effect: pick the "TAN" version of any debrid stream and hear it
//! normalized.
//!
//! Limitations (v1, by design): only streams that resolve to a direct URL can
//! be wrapped (a raw torrent/`infoHash` has no URL to pull, so it's skipped -
//! use a debrid service); the source is read twice (once for video, once for
//! audio); surround is downmixed to stereo; and the live MPEG-TS stream does
//! not support seeking. All are documented in the README.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::Arc;

struct Config {
    port: u16,
    bind: String,
    upstreams: Vec<String>,
    profile: String,
    ffmpeg: String,
    self_exe: String,
    /// Directory containing `tan_ladspa.so`, if found. When set, transcodes in
    /// a single ffmpeg pass via the LADSPA filter (one source read) instead of
    /// the two-read pcm-filter pipe.
    ladspa_dir: Option<String>,
    /// Cap on TAN variants offered per title, so a debrid addon's long stream
    /// list doesn't flood Stremio with duplicates.
    max_streams: usize,
}

pub fn run(args: &[String]) -> ! {
    let upstreams: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == "--upstream")
        .filter_map(|(i, _)| args.get(i + 1))
        .map(|u| normalize_upstream(u))
        .collect();
    if upstreams.is_empty() {
        eprintln!(
            "tan-stremio proxy: at least one --upstream <addon url> is required.\n\n\
             Point it at your debrid-configured stream addon (the URL you'd paste\n\
             into Stremio), e.g. your Torrentio 'Install' URL:\n  \
             tan-stremio proxy --upstream \"https://torrentio.strem.fun/<your-config>/manifest.json\"\n\n\
             Optional: --port <n> (default 5870), --bind <addr> (default 127.0.0.1),\n  \
             --profile <universal|movie|music|speech|night|game> (default movie),\n  \
             --ffmpeg <path>."
        );
        std::process::exit(1);
    }
    let self_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "tan-stremio".to_string());
    let ladspa_dir = resolve_ladspa(flag(args, "--ladspa"), &self_exe);
    let cfg = Config {
        port: flag(args, "--port").and_then(|s| s.parse().ok()).unwrap_or(5870),
        bind: flag(args, "--bind").unwrap_or_else(|| "127.0.0.1".to_string()),
        upstreams,
        profile: flag(args, "--profile").unwrap_or_else(|| "movie".to_string()),
        ffmpeg: flag(args, "--ffmpeg").unwrap_or_else(|| "ffmpeg".to_string()),
        self_exe,
        ladspa_dir,
        max_streams: flag(args, "--max-streams").and_then(|s| s.parse().ok()).filter(|&n| n > 0).unwrap_or(8),
    };
    if crate::profile_by_name(&cfg.profile).is_none() {
        eprintln!("tan-stremio proxy: unknown profile '{}'", cfg.profile);
        std::process::exit(1);
    }

    let addr = format!("{}:{}", cfg.bind, cfg.port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("tan-stremio proxy: couldn't bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("TAN Stremio proxy addon listening on http://{addr}");
    println!("Install in Stremio with manifest URL: http://{addr}/manifest.json");
    println!("Wrapping upstream addon(s):");
    for u in &cfg.upstreams {
        println!("  {u}");
    }
    match &cfg.ladspa_dir {
        Some(d) => println!("Single-pass transcode: ON (LADSPA plugin at {d})"),
        None => println!("Single-pass transcode: off (two-read pipe; build tan-stremio/ladspa for single-pass)"),
    }
    if cfg.bind != "127.0.0.1" && cfg.bind != "localhost" {
        eprintln!("warning: bound to {}, not localhost - no authentication.", cfg.bind);
    }

    let cfg = Arc::new(cfg);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cfg = Arc::clone(&cfg);
                std::thread::spawn(move || {
                    if let Err(e) = handle(&stream, &cfg) {
                        if e.kind() != io::ErrorKind::BrokenPipe {
                            eprintln!("connection error: {e}");
                        }
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    std::process::exit(0);
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// Accept either the full `.../manifest.json` install URL or a bare base URL;
/// store the base (no trailing slash, no `/manifest.json`).
fn normalize_upstream(u: &str) -> String {
    let u = u.trim().trim_end_matches('/');
    u.strip_suffix("/manifest.json").unwrap_or(u).trim_end_matches('/').to_string()
}

// --- HTTP handling ----------------------------------------------------

struct Req {
    method: String,
    path: String,
}

fn read_req<R: BufRead>(r: &mut R) -> io::Result<Req> {
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty request"));
    }
    let mut parts = line.trim_end().split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 || h.trim_end().is_empty() {
            break;
        }
    }
    Ok(Req { method, path })
}

fn cors_json<W: Write>(w: &mut W, body: &str) -> io::Result<()> {
    write!(
        w,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    w.write_all(body.as_bytes())
}

fn plain<W: Write>(w: &mut W, status: u16, body: &str) -> io::Result<()> {
    let reason = if status == 404 { "Not Found" } else if status == 502 { "Bad Gateway" } else { "OK" };
    write!(
        w,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    w.write_all(body.as_bytes())
}

fn handle(stream: &TcpStream, cfg: &Config) -> io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream);
    let req = match read_req(&mut reader) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    let mut w = stream;

    if req.method == "OPTIONS" {
        return write!(
            w,
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\n\
             Access-Control-Allow-Headers: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
    }
    if req.method != "GET" && req.method != "HEAD" {
        return plain(&mut w, 404, "not found");
    }

    let path = req.path.as_str();
    if path == "/" || path == "/manifest.json" {
        return cors_json(&mut w, &manifest_json());
    }
    // /stream/<type>/<id>.json
    if let Some(rest) = path.strip_prefix("/stream/").and_then(|s| s.strip_suffix(".json")) {
        let mut it = rest.splitn(2, '/');
        let ctype = it.next().unwrap_or("");
        let id = percent_decode(it.next().unwrap_or(""));
        return cors_json(&mut w, &streams_for(cfg, ctype, &id));
    }
    // /p/<base64url(src)>/<profile>/tan.ts  -> live transcode
    if let Some(rest) = path.strip_prefix("/p/") {
        let mut parts = rest.split('/');
        let b64 = parts.next().unwrap_or("");
        let profile = parts.next().unwrap_or(&cfg.profile);
        let Some(src) = b64url_decode(b64).ok().and_then(|b| String::from_utf8(b).ok()) else {
            return plain(&mut w, 404, "bad source");
        };
        if !(src.starts_with("http://") || src.starts_with("https://")) {
            return plain(&mut w, 404, "unsupported source");
        }
        return transcode(&mut w, cfg, &src, profile);
    }

    plain(&mut w, 404, "not found")
}

// --- Addon resources --------------------------------------------------

fn manifest_json() -> String {
    format!(
        r#"{{"id":"com.tan.stremio.proxy","version":"{}","name":"TAN Normalizer (live)","description":"Adds a real-time TAN-normalized variant of your debrid streams. Requires an upstream stream addon whose streams resolve to direct URLs (a debrid service). Torrent-only streams are skipped. Processing runs locally via ffmpeg.","resources":["stream"],"types":["movie","series"],"idPrefixes":["tt"],"catalogs":[]}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn streams_for(cfg: &Config, ctype: &str, id: &str) -> String {
    if ctype != "movie" && ctype != "series" {
        return r#"{"streams":[]}"#.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    'outer: for base in &cfg.upstreams {
        let url = format!("{base}/stream/{ctype}/{}.json", url_encode(id));
        let Some(body) = http_get(&url) else { continue };
        for s in parse_streams(&body) {
            if out.len() >= cfg.max_streams {
                break 'outer;
            }
            // Only wrap streams that resolve to a direct URL; skip torrents.
            let Some(src) = s.url else { continue };
            if !(src.starts_with("http://") || src.starts_with("https://")) {
                continue;
            }
            let label = s.title.or(s.name).unwrap_or_default();
            let label = first_line(&label);
            let proxy_url = format!(
                "http://127.0.0.1:{}/p/{}/{}/tan.ts",
                cfg.port,
                b64url_encode(src.as_bytes()),
                cfg.profile
            );
            let title = if label.is_empty() {
                "TAN normalized".to_string()
            } else {
                format!("TAN | {label}")
            };
            out.push(format!(
                r#"{{"name":"TAN {}","title":"{}","url":"{}","behaviorHints":{{"notWebReady":true,"bingeGroup":"tan-{}"}}}}"#,
                json_escape(&cfg.profile),
                json_escape(&title),
                json_escape(&proxy_url),
                json_escape(&cfg.profile)
            ));
        }
    }
    format!(r#"{{"streams":[{}]}}"#, out.join(","))
}

// --- Live transcode ---------------------------------------------------

fn resolve_ladspa(flag_dir: Option<String>, self_exe: &str) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(d) = flag_dir {
        candidates.push(std::path::PathBuf::from(d));
    }
    if let Some(exe_dir) = std::path::Path::new(self_exe).parent() {
        candidates.push(exe_dir.join("ladspa"));
        candidates.push(exe_dir.to_path_buf());
        // dev tree: target/<profile>/tan-stremio.exe -> ../../tan-stremio/ladspa
        candidates.push(exe_dir.join("..").join("..").join("tan-stremio").join("ladspa"));
    }
    for c in candidates {
        if c.join("tan_ladspa.so").is_file() {
            return c.canonicalize().ok().map(|p| p.to_string_lossy().into_owned());
        }
    }
    None
}

fn transcode(w: &mut &TcpStream, cfg: &Config, src: &str, profile: &str) -> io::Result<()> {
    let profile = if crate::profile_by_name(profile).is_some() { profile } else { &cfg.profile };
    // Single-pass LADSPA when the plugin is present (all profiles, selected via
    // its control port); otherwise the two-read pcm-filter pipe.
    if let Some(dir) = cfg.ladspa_dir.clone() {
        return transcode_ladspa(w, cfg, src, &dir, profile);
    }
    transcode_pipe(w, cfg, src, profile)
}

/// profile name -> tan-ffi profile_id (must match tan-ffi::profile_from_id and
/// the tan_ladspa control port).
fn profile_id(name: &str) -> u32 {
    match name {
        "music" => 1,
        "universal" => 2,
        "speech" => 3,
        "night" => 4,
        "game" => 5,
        _ => 0, // movie
    }
}

/// One ffmpeg pass: source read once, video copied, TAN applied to stereo via
/// the LADSPA filter, remuxed to live MPEG-TS.
fn transcode_ladspa(w: &mut &TcpStream, cfg: &Config, src: &str, dir: &str, profile: &str) -> io::Result<()> {
    let af = format!(
        "aresample=48000,aformat=channel_layouts=stereo,ladspa=file=./tan_ladspa.so:plugin=tan:controls=c0={}",
        profile_id(profile)
    );
    let mut mux = Command::new(&cfg.ffmpeg)
        .current_dir(dir)
        .args([
            "-hide_banner", "-loglevel", "error", "-i", src,
            "-map", "0:v:0", "-c:v", "copy", "-map", "0:a:0",
            "-af", af.as_str(),
            "-c:a", "aac", "-b:a", "192k", "-f", "mpegts", "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut out = mux.stdout.take().expect("mux stdout");
    write!(
        w,
        "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
    )?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        match out.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if w.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let _ = mux.kill();
    let _ = mux.wait();
    Ok(())
}

fn transcode_pipe(w: &mut &TcpStream, cfg: &Config, src: &str, profile: &str) -> io::Result<()> {
    // Stage 1: decode source audio to raw stereo f32le.
    let mut dec = Command::new(&cfg.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-i", src,
               "-vn", "-f", "f32le", "-ar", "48000", "-ac", "2", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let dec_out = dec.stdout.take().expect("dec stdout");

    // Stage 2: TAN filter (this same binary, pcm-filter mode).
    let mut tan = Command::new(&cfg.self_exe)
        .args(["pcm-filter", "--rate", "48000", "--channels", "2", "--profile", profile])
        .stdin(Stdio::from(dec_out))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let tan_out = tan.stdout.take().expect("tan stdout");

    // Stage 3: mux copied video with the TAN'd audio into live MPEG-TS.
    let mut mux = Command::new(&cfg.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error",
               "-i", src,
               "-f", "f32le", "-ar", "48000", "-ac", "2", "-i", "-",
               "-map", "0:v:0", "-map", "1:a:0",
               "-c:v", "copy", "-c:a", "aac", "-b:a", "192k",
               "-f", "mpegts", "-"])
        .stdin(Stdio::from(tan_out))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut mux_out = mux.stdout.take().expect("mux stdout");

    // Stream the muxed output to the client with no Content-Length (live).
    write!(
        w,
        "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
    )?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        match mux_out.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if w.write_all(&buf[..n]).is_err() {
                    break; // client disconnected
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    // Tear the pipeline down so we don't leak ffmpeg processes on disconnect.
    let _ = mux.kill();
    let _ = tan.kill();
    let _ = dec.kill();
    let _ = mux.wait();
    let _ = tan.wait();
    let _ = dec.wait();
    Ok(())
}

fn http_get(url: &str) -> Option<String> {
    // Shell out to curl (in-box on Windows 10+/most systems) for HTTPS - avoids
    // pulling a TLS stack into this otherwise dependency-light crate.
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "20", url])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

// --- Minimal JSON (enough to read an addon's stream list) -------------

#[derive(Debug)]
pub struct UpstreamStream {
    pub url: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
}

/// Extract the `streams` array's objects, pulling `url`, `name`, `title`.
/// Tolerant: walks a parsed JSON value rather than regexing raw text.
pub fn parse_streams(body: &str) -> Vec<UpstreamStream> {
    let mut p = JsonParser::new(body);
    let Some(val) = p.parse_value() else { return Vec::new() };
    let Json::Object(obj) = val else { return Vec::new() };
    let Some(Json::Array(arr)) = obj.into_iter().find(|(k, _)| k == "streams").map(|(_, v)| v) else {
        return Vec::new();
    };
    arr.into_iter()
        .filter_map(|v| match v {
            Json::Object(fields) => {
                let mut s = UpstreamStream { url: None, name: None, title: None };
                for (k, val) in fields {
                    if let Json::String(str_val) = val {
                        match k.as_str() {
                            "url" => s.url = Some(str_val),
                            "name" => s.name = Some(str_val),
                            "title" => s.title = Some(str_val),
                            _ => {}
                        }
                    }
                }
                Some(s)
            }
            _ => None,
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

struct JsonParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn new(s: &'a str) -> Self {
        Self { b: s.as_bytes(), i: 0 }
    }
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn parse_value(&mut self) -> Option<Json> {
        self.ws();
        let c = *self.b.get(self.i)?;
        match c {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => self.parse_string().map(Json::String),
            b't' => self.lit("true", Json::Bool(true)),
            b'f' => self.lit("false", Json::Bool(false)),
            b'n' => self.lit("null", Json::Null),
            _ => self.parse_number(),
        }
    }
    fn lit(&mut self, s: &str, v: Json) -> Option<Json> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Some(v)
        } else {
            None
        }
    }
    fn parse_object(&mut self) -> Option<Json> {
        self.i += 1; // {
        let mut out = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Some(Json::Object(out));
        }
        loop {
            self.ws();
            let key = self.parse_string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') {
                return None;
            }
            self.i += 1;
            let val = self.parse_value()?;
            out.push((key, val));
            self.ws();
            match self.b.get(self.i) {
                Some(&b',') => {
                    self.i += 1;
                    continue;
                }
                Some(&b'}') => {
                    self.i += 1;
                    return Some(Json::Object(out));
                }
                _ => return None,
            }
        }
    }
    fn parse_array(&mut self) -> Option<Json> {
        self.i += 1; // [
        let mut out = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Some(Json::Array(out));
        }
        loop {
            let val = self.parse_value()?;
            out.push(val);
            self.ws();
            match self.b.get(self.i) {
                Some(&b',') => {
                    self.i += 1;
                    continue;
                }
                Some(&b']') => {
                    self.i += 1;
                    return Some(Json::Array(out));
                }
                _ => return None,
            }
        }
    }
    fn parse_string(&mut self) -> Option<String> {
        if self.b.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out = String::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = self.b.get(self.i..self.i + 4)?;
                            let cp = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                            self.i += 4;
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        _ => return None,
                    }
                }
                _ => {
                    // Bytes >= 0x80 are UTF-8 continuation; collect raw and
                    // rebuild. Simplest: push as part of a byte buffer. Here we
                    // handle ASCII directly and multibyte via from_utf8 later
                    // would be complex; instead push the byte through a small
                    // UTF-8 accumulator.
                    if c < 0x80 {
                        out.push(c as char);
                    } else {
                        // Determine sequence length and consume continuation bytes.
                        let len = if c >= 0xF0 { 4 } else if c >= 0xE0 { 3 } else { 2 };
                        let start = self.i - 1;
                        let end = start + len;
                        if end <= self.b.len() {
                            if let Ok(s) = std::str::from_utf8(&self.b[start..end]) {
                                out.push_str(s);
                                self.i = end;
                            } else {
                                out.push('\u{fffd}');
                            }
                        } else {
                            out.push('\u{fffd}');
                        }
                    }
                }
            }
        }
        None
    }
    fn parse_number(&mut self) -> Option<Json> {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(self.b[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
        {
            self.i += 1;
        }
        std::str::from_utf8(&self.b[start..self.i]).ok()?.parse().ok().map(Json::Number)
    }
}

// --- helpers ----------------------------------------------------------

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(h) = std::str::from_utf8(&b[i + 1..i + 3]) {
                if let Ok(byte) = u8::from_str_radix(h, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encode a Stremio content id for use in an upstream path segment.
/// Ids are ASCII (`tt123`, `tt123:1:2`); encode the `:` and anything unsafe.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 63) as usize] as char);
        }
    }
    out
}

fn b64url_decode(s: &str) -> Result<Vec<u8>, ()> {
    let val = |c: u8| -> Result<u32, ()> {
        B64.iter().position(|&x| x == c).map(|p| p as u32).ok_or(())
    };
    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_roundtrips() {
        for s in ["https://x.example/a b?c=1&d=2", "", "a", "ab", "abc", "abcd"] {
            let e = b64url_encode(s.as_bytes());
            assert!(!e.contains('+') && !e.contains('/') && !e.contains('='));
            assert_eq!(b64url_decode(&e).unwrap(), s.as_bytes());
        }
    }

    #[test]
    fn parses_streams_and_pulls_direct_urls() {
        let body = r#"{"streams":[
            {"name":"Torrentio","title":"Movie 1080p","url":"https://rd.example/abc.mkv"},
            {"name":"Torrentio","title":"torrent only","infoHash":"deadbeef"},
            {"url":"http://plain.example/x.mp4"}
        ]}"#;
        let s = parse_streams(body);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].url.as_deref(), Some("https://rd.example/abc.mkv"));
        assert_eq!(s[1].url, None);
        assert_eq!(s[2].url.as_deref(), Some("http://plain.example/x.mp4"));
    }

    #[test]
    fn json_parser_handles_nested_and_escapes() {
        let body = r#"{"a":{"b":[1,2,"x\"y"]},"streams":[{"url":"u\/v","name":"n"}]}"#;
        let s = parse_streams(body);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].url.as_deref(), Some("u/v"));
        assert_eq!(s[0].name.as_deref(), Some("n"));
    }

    #[test]
    fn normalize_upstream_strips_manifest_and_slashes() {
        assert_eq!(normalize_upstream("https://t.fun/cfg/manifest.json"), "https://t.fun/cfg");
        assert_eq!(normalize_upstream("https://t.fun/cfg/"), "https://t.fun/cfg");
        assert_eq!(normalize_upstream("https://t.fun/cfg"), "https://t.fun/cfg");
    }

    #[test]
    fn manifest_declares_stream_resource_for_imdb() {
        let m = manifest_json();
        assert!(m.contains(r#""resources":["stream"]"#));
        assert!(m.contains(r#""idPrefixes":["tt"]"#));
        assert!(m.contains(r#""types":["movie","series"]"#));
    }

    #[test]
    fn url_encode_escapes_the_series_colon() {
        assert_eq!(url_encode("tt123:1:2"), "tt123%3A1%3A2");
        assert_eq!(url_encode("tt123"), "tt123");
    }
}
