# Deploy the TAN Stremio addon on a VPS

Self-contained: one Docker image (the Rust addon + the TAN LADSPA plugin +
ffmpeg + curl) fronted by Caddy for automatic HTTPS. Once it's up, any Stremio
client - **Android TV**, web, desktop, mobile - installs a single URL and it
just works, with nothing running near the TV.

## What it does

TAN is a man-in-the-middle transcoder: it wraps your debrid stream addon and,
for each stream, offers a "TAN" variant. When you play it, this server pulls
the original from debrid, copies the video, runs the audio through TAN live,
and serves seekable HLS. So the video's full bandwidth flows through this box -
size the VPS/bandwidth accordingly, and keep it private (it carries your debrid
key and relays your streams).

## Prerequisites

- A small VPS (1-2 vCPU is plenty; audio-only encode + video copy). Pick a host
  with enough bandwidth for your viewing - the movie streams through it.
- A **domain** (or subdomain) with a DNS **A record** pointing at the VPS IP.
  Needed for HTTPS (Stremio web requires it; native apps like it too).
- Docker + Docker Compose on the VPS.

## Steps

```sh
# on the VPS, in a checkout of this repo:
cd tan-stremio/deploy
cp .env.example .env
nano .env            # set DOMAIN, TAN_UPSTREAM (your debrid Torrentio URL), TAN_SECRET
docker compose up -d --build
```

Caddy fetches a Let's Encrypt cert for `DOMAIN` automatically. Then, in Stremio
(any device), add the addon:

```
https://<DOMAIN>/<TAN_SECRET>/manifest.json
```

Open a movie, pick a **"TAN"** stream. Change `TAN_PROFILE` and
`docker compose up -d` to re-roll the sound.

## Notes / limits

- **Only debrid (direct-URL) streams** are wrapped; raw torrents are skipped.
- **Bandwidth**: the whole movie transits the VPS (video is copied, not
  re-encoded, so CPU is light but bytes are not). This is inherent to any
  audio-processing MITM - there's no way to change the audio without the media
  passing through.
- **Legal**: you're relaying your own debrid streams through your own box. A
  private instance for yourself is one thing; running a public shared one for
  strangers is a different, riskier proposition - that's on you.
- **Seeking** works within the transcoded-so-far range (linear HLS).
- Surround is downmixed to stereo (for now).

## Updating

```sh
git pull
docker compose up -d --build
```
