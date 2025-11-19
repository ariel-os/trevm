#![no_std]
pub mod wasm;

// Required by wasmtime
static mut TLS_PTR: *mut u8 = core::ptr::null_mut();

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get() -> *mut u8 {
    unsafe { TLS_PTR }
}

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(ptr: *mut u8) {
    unsafe { TLS_PTR = ptr }
}
