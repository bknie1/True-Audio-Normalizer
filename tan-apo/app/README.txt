TAN - True Audio Normalizer (user-mode)
=======================================

TAN evens out loud/quiet audio in real time. This version processes any app you
route through a virtual audio cable and plays the result to your speakers or
headphones - no special drivers from us, works with Secure Boot on.

FIRST RUN
1. Open "TAN Control" (Start Menu or desktop).
2. If it says the virtual cable is not installed, click "Install VB-CABLE"
   (free, Microsoft-signed). Follow its installer, and reboot if it asks.
3. Pick a Profile (e.g. Movie) and your Output device (or leave System default).
4. Click "Turn TAN On".

USING IT
- In Windows Sound settings (or an app's own audio settings), set the app you
  want normalized to play to "CABLE Input".
- You'll hear the normalized result on your chosen output.
- Turn TAN Off (or close it) to go back to raw audio.

STREMIO (streamed movies)
TAN Control also has a "Stremio addon" section for normalizing movies you
stream in Stremio via a debrid service (Real-Debrid/AllDebrid/TorBox/etc.):
1. In Stremio, configure Torrentio with your debrid, and copy its "Install"
   URL from https://torrentio.strem.fun/configure
2. Paste that URL into TAN Control's Stremio box and click Start.
3. In Stremio, add the shown manifest URL (http://127.0.0.1:5870/manifest.json)
   as an addon. Open a movie and pick a "TAN" stream - it's normalized live.
Only debrid (direct-URL) streams can be normalized; raw torrents are skipped.

NOTES
- This installer is not code-signed yet, so Windows SmartScreen may warn on
  first run - choose "More info" then "Run anyway".
- VB-CABLE is by VB-Audio (vb-audio.com), free for personal use.
