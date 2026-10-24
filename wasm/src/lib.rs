//! The isolex compiler as a WebAssembly module, for hosts that embed it
//! (a Go server under wazero, a browser): build it with
//!
//! ```sh
//! cargo build --release --locked -p isolex-wasm --target wasm32-unknown-unknown
//! ```
//!
//! It imports nothing, so a host grants it nothing: no clock, no files,
//! no randomness. Memory changes hands through four exports:
//!
//! - `isolex_alloc(len) -> ptr` reserves `len` bytes for the host to fill;
//! - `isolex_compile(ptr, len) -> u64` reads that many bytes of input JSON
//!   (see `isolex::json`), frees them, and returns the output JSON as
//!   `ptr << 32 | len`;
//! - `isolex_free(ptr, len)` frees what `isolex_compile` returned;
//! - `isolex_version() -> u64`, the crate version, also as `ptr << 32 | len`,
//!   which is static and never freed.
//!
//! A panic traps, which a host reports as the compiler failing rather than
//! the lexicon.

#[no_mangle]
pub extern "C" fn isolex_alloc(len: u32) -> u32 {
    Box::into_raw(vec![0u8; len as usize].into_boxed_slice()) as *mut u8 as u32
}

/// # Safety
/// `ptr` and `len` must be exactly what `isolex_alloc` or
/// `isolex_compile` handed out, and not already freed.
#[no_mangle]
pub unsafe extern "C" fn isolex_free(ptr: u32, len: u32) {
    drop(take(ptr, len));
}

/// # Safety
/// `ptr` must come from `isolex_alloc(len)` and hold `len` bytes the host
/// wrote; it is freed here.
#[no_mangle]
pub unsafe extern "C" fn isolex_compile(ptr: u32, len: u32) -> u64 {
    let input = take(ptr, len);
    let out = isolex::json::compile_json(&input);
    drop(input);
    pack(out.into_bytes().into_boxed_slice())
}

#[no_mangle]
pub extern "C" fn isolex_version() -> u64 {
    let v = env!("CARGO_PKG_VERSION");
    ((v.as_ptr() as u64) << 32) | v.len() as u64
}

/// Ownership of a buffer handed out by `isolex_alloc` or `pack`, back.
unsafe fn take(ptr: u32, len: u32) -> Box<[u8]> {
    Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        ptr as *mut u8,
        len as usize,
    ))
}

fn pack(bytes: Box<[u8]>) -> u64 {
    let len = bytes.len() as u64;
    let ptr = Box::into_raw(bytes) as *mut u8 as u64;
    (ptr << 32) | len
}
