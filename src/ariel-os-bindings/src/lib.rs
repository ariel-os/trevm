#![no_std]
pub mod wasm;

// Required by wasmtime
// After Wasmtime 48, the TLS size changed
static mut TLS_PTR: [*mut u8; 2] = [core::ptr::null_mut(); 2];

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get(slot: usize) -> *mut u8 {
    unsafe { TLS_PTR[slot] }
}

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(slot: usize, ptr: *mut u8) {
    unsafe { TLS_PTR[slot] = ptr }
}
