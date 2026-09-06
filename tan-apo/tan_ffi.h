// C declarations for the tan-ffi streaming ABI (see tan-ffi/src/lib.rs).
// Link against tan.lib (cargo build -p tan-ffi produces target/<profile>/tan.lib).
#pragma once
#include <stddef.h>
#include <stdint.h>

extern "C" {

// Opaque handle to a live Normalizer. profile_id: 0 = movie, 1 = music.
typedef struct TanNormalizer TanNormalizer;

TanNormalizer* tan_normalizer_new(uint32_t sample_rate, uint32_t channels, uint32_t profile_id);

// Level one interleaved block in place. `len` is total floats (frames * channels).
void tan_normalizer_process(TanNormalizer* handle, float* interleaved, size_t len);

void tan_normalizer_free(TanNormalizer* handle);

} // extern "C"
