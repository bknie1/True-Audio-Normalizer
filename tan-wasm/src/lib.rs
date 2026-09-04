//! WebAssembly exports for tan-core, using plain C-ABI functions and raw
//! pointers into wasm linear memory. The JavaScript side allocates a buffer,
//! copies samples in, calls a function, and copies results out - no
//! framework glue needed.

use tan_core::{Normalizer, Profile};

fn profile_from_id(id: u32) -> Profile {
    match id {
        1 => Profile::music(),
        _ => Profile::movie(),
    }
}

/// Allocate a buffer of `len` f32s inside wasm memory; returns its pointer.
#[unsafe(no_mangle)]
pub extern "C" fn tan_alloc(len: usize) -> *mut f32 {
    let mut buf = Vec::<f32>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Free a buffer previously returned by tan_alloc with the same length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tan_free(ptr: *mut f32, len: usize) {
    unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
}

/// Offline two-pass normalization, in place over interleaved samples.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tan_normalize_offline(
    ptr: *mut f32,
    len: usize,
    sample_rate: u32,
    channels: u32,
    profile_id: u32,
) {
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
    // normalize_offline needs a Vec (it flushes limiter latency through the
    // end), so process a copy and write the result back over the input.
    let mut work = slice.to_vec();
    tan_core::normalize_offline(
        &mut work,
        sample_rate,
        channels as usize,
        profile_from_id(profile_id),
    );
    slice.copy_from_slice(&work[..len]);
}

/// Create a streaming normalizer for live processing; returns an opaque handle.
#[unsafe(no_mangle)]
pub extern "C" fn tan_normalizer_new(sample_rate: u32, channels: u32, profile_id: u32) -> *mut Normalizer {
    Box::into_raw(Box::new(Normalizer::new(
        sample_rate,
        channels as usize,
        profile_from_id(profile_id),
    )))
}

/// Process one interleaved block in place through a live normalizer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tan_normalizer_process(handle: *mut Normalizer, ptr: *mut f32, len: usize) {
    let normalizer = unsafe { &mut *handle };
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
    normalizer.process(slice);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tan_normalizer_free(handle: *mut Normalizer) {
    unsafe { drop(Box::from_raw(handle)) };
}
