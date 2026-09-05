# tan-server

TAN as a local web API. Send it a WAV file, get back the TAN-processed WAV.

This exists as the shared foundation for platform integrations - a Jellyfin
plugin, a future Stremio-facing proxy, a shell script, anything that can make
an HTTP request - so the actual DSP integration is built once, here, instead
of re-embedded per platform in whatever language that platform speaks.

## Running it

```
cargo run -p tan-server -- [--port 5859] [--bind 127.0.0.1]
```

Binds to `127.0.0.1` (localhost only) by default. **This API has no
authentication** - only bind elsewhere if something in front of it (a reverse
proxy, a firewall rule) is providing that.

## API

- `GET /health` - `TAN server v<version>`, for a quick liveness check.
- `POST /normalize?profile=<name>` - body is a WAV file (PCM, 8 or 16-bit);
  response is the processed WAV, same format. `profile` is one of `universal`
  (default), `movie`, `music`, `speech`, `night`, `game` - see `tan-core`'s
  `Profile` presets.

```
curl -X POST "http://127.0.0.1:5859/normalize?profile=movie" \
  --data-binary @input.wav -o output.wav
```

## Design notes

Hand-rolled HTTP/1.1 directly on `std::net::TcpListener` - no web framework,
no async runtime, consistent with the rest of the project (hand-written WAV
codec, hand-written ring buffer). One request per connection
(`Connection: close`), a `Content-Length` body required on requests, no
chunked transfer-encoding. That's a deliberate, honest scope: this is a small
local processing endpoint, not a general-purpose web server, and it doesn't
try to be one.

Processing is the same offline two-pass engine `tan-cli` and the WASM demo
use (`tan_core::normalize_offline`) - not the live streaming `Normalizer` -
so a file goes through in one request/response instead of needing a
persistent connection.
