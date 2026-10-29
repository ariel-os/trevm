use wasmtime::ResourceLimiter;

/// The DefaultResourceLimiter does not limit resource consumption by a capsule, but simply allows any
/// request.
pub struct DefaultResourceLimiter;

impl ResourceLimiter for DefaultResourceLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}
