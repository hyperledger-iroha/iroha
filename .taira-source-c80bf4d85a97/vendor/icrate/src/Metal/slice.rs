use crate::Metal;
use crate::common::*;

#[allow(dead_code)]
fn slice_to_ptr_count<T>(slice: &[T]) -> (NonNull<T>, usize) {
    let ptr: *const T = slice.as_ptr();
    let ptr: *mut T = ptr as *mut T;
    // SAFETY: Slice pointers are always non-null
    let ptr = unsafe { NonNull::new_unchecked(ptr) };
    (ptr, slice.len())
}

#[cfg(feature = "Metal_MTLRenderCommandEncoder")]
impl Metal::MTLRenderCommandEncoder {
    #[cfg(feature = "Metal_MTLViewport")]
    /// Set the viewport state from a slice of [`MTLViewport`] descriptors.
    ///
    /// This copies the descriptors immediately; the slice does not need to
    /// remain alive after the call returns.
    pub fn setViewports(&self, viewports: &[Metal::MTLViewport]) {
        let (ptr, count) = slice_to_ptr_count(viewports);
        // SAFETY: `slice_to_ptr_count` returns a non-null pointer backed by the
        // provided slice for the duration of this call, and Metal only reads
        // the viewport descriptors synchronously.
        unsafe { self.setViewports_count(ptr, count) }
    }

    #[cfg(feature = "Metal_MTLScissorRect")]
    /// Set the scissor rect state from a slice of [`MTLScissorRect`].
    ///
    /// Metal consumes the rect descriptors synchronously; the slice does not
    /// need to outlive the call.
    pub fn setScissorRects(&self, scissor_rects: &[Metal::MTLScissorRect]) {
        let (ptr, count) = slice_to_ptr_count(scissor_rects);
        unsafe { self.setScissorRects_count(ptr, count) }
    }

    #[cfg(feature = "Metal_MTLVertexAmplificationViewMapping")]
    /// Configure vertex amplification using the provided view mappings.
    ///
    /// Passing an empty slice clears any previously configured view mappings.
    pub fn setVertexAmplificationViewMappings(
        &self,
        view_mappings: &[Metal::MTLVertexAmplificationViewMapping],
    ) {
        if view_mappings.is_empty() {
            unsafe { self.setVertexAmplificationCount_viewMappings(0, core::ptr::null_mut()) }
        } else {
            let (ptr, count) = slice_to_ptr_count(view_mappings);
            unsafe {
                self.setVertexAmplificationCount_viewMappings(count, ptr.as_ptr());
            }
        }
    }
}

// Additional helpers can be added here for other pointer/length pairs as
// needed.
