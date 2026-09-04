// Runs on the browser's real-time audio thread. Receives the TAN wasm module
// bytes from the main thread, instantiates it, and streams every 128-frame
// audio quantum through the live normalizer. Until the engine is ready it
// passes audio through untouched.
class TanProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.engine = null;
    this.port.onmessage = (e) => {
      if (e.data.wasmBytes) {
        WebAssembly.instantiate(e.data.wasmBytes, {}).then((result) => {
          this.wasm = result.instance.exports;
          this.profileId = e.data.profileId | 0;
          this.port.postMessage({ ready: true });
        });
      }
    };
  }

  ensureEngine(channels) {
    if (this.engine && this.channels === channels) return;
    if (this.engine) this.wasm.tan_normalizer_free(this.engine);
    this.channels = channels;
    this.engine = this.wasm.tan_normalizer_new(sampleRate, channels, this.profileId);
    this.bufLen = 128 * channels;
    this.bufPtr = this.wasm.tan_alloc(this.bufLen);
  }

  process(inputs, outputs) {
    const input = inputs[0];
    const output = outputs[0];
    if (!input.length) return true;

    if (!this.wasm) {
      for (let ch = 0; ch < output.length; ch++) {
        output[ch].set(input[ch % input.length]);
      }
      return true;
    }

    const channels = input.length;
    this.ensureEngine(channels);

    // Views into wasm memory must be recreated each call: the buffer object
    // is invalidated whenever wasm memory grows.
    const mem = new Float32Array(this.wasm.memory.buffer, this.bufPtr, this.bufLen);
    const frames = input[0].length;
    for (let i = 0; i < frames; i++) {
      for (let ch = 0; ch < channels; ch++) {
        mem[i * channels + ch] = input[ch][i];
      }
    }
    this.wasm.tan_normalizer_process(this.engine, this.bufPtr, frames * channels);
    const out = new Float32Array(this.wasm.memory.buffer, this.bufPtr, this.bufLen);
    for (let i = 0; i < frames; i++) {
      for (let ch = 0; ch < output.length; ch++) {
        output[ch][i] = out[i * channels + (ch % channels)];
      }
    }
    return true;
  }
}

registerProcessor("tan-processor", TanProcessor);
