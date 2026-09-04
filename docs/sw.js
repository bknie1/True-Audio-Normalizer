// Minimal service worker: no offline caching (the wasm engine already runs
// fully client-side), it exists only to satisfy browsers' installability
// requirement for "Add to Home Screen".
self.addEventListener("fetch", () => {});
