use crate::BodyExt as _;

use bytes::Buf;
use http_body::{Body, Frame, SizeHint};
use std::{
    alloc::{alloc, dealloc, Layout},
    fmt,
    mem::ManuallyDrop,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

/// A boxed [`Body`] trait object.
pub struct BoxBody<D, E> {
    inner: Pin<Box<dyn Body<Data = D, Error = E> + Send + Sync + 'static>>,
}

/// A boxed [`Body`] trait object that is !Sync.
pub struct UnsyncBoxBody<D, E> {
    inner: ManuallyDrop<Pin<Box<dyn Body<Data = D, Error = E> + Send + 'static>>>,
    reclaim: Option<ErasedReclaim>,
}

// The separately allocated token must outlive the body's allocation, including
// Box's unwind cleanup when the body's destructor panics.
struct ErasedReclaim {
    pointer: NonNull<()>,
    release: unsafe fn(NonNull<()>),
}

// Only try_new_with_reclaim constructs this value, and it requires R: Send.
unsafe impl Send for ErasedReclaim {}

impl Drop for ErasedReclaim {
    fn drop(&mut self) {
        // SAFETY: the constructor stores exactly one initialized R at pointer
        // and its matching release function. This token is not cloned.
        unsafe { (self.release)(self.pointer) };
    }
}

unsafe fn release_reclaim<R>(pointer: NonNull<()>) {
    let pointer = pointer.cast::<R>();
    // SAFETY: the constructor initialized R in this unique allocation. Moving
    // R onto the stack makes its storage available before running R::drop.
    let reclaim = unsafe { pointer.as_ptr().read() };
    let layout = Layout::new::<R>();
    if layout.size() != 0 {
        // SAFETY: this is the original allocation and exact layout of R.
        unsafe { dealloc(pointer.as_ptr().cast::<u8>(), layout) };
    }
    drop(reclaim);
}

fn try_allocate<T>() -> Option<NonNull<T>> {
    let layout = Layout::new::<T>();
    if layout.size() == 0 {
        Some(NonNull::dangling())
    } else {
        // SAFETY: layout is nonzero and has T's valid size and alignment.
        NonNull::new(unsafe { alloc(layout) }.cast::<T>())
    }
}

impl<D, E> BoxBody<D, E> {
    /// Create a new `BoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: Body<Data = D, Error = E> + Send + Sync + 'static,
        D: Buf,
    {
        Self {
            inner: Box::pin(body),
        }
    }
}

impl<D, E> Drop for UnsyncBoxBody<D, E> {
    fn drop(&mut self) {
        // This initialized local is an unwind guard as well as the ordinary
        // release owner. Box's cleanup deallocates the body before an unwind
        // reaches this frame and drops the guard.
        let reclaim = self.reclaim.take();
        // SAFETY: inner was initialized by either constructor and is dropped
        // exactly once here. ManuallyDrop suppresses automatic field cleanup.
        unsafe { ManuallyDrop::drop(&mut self.inner) };
        drop(reclaim);
    }
}

impl<D, E> fmt::Debug for BoxBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxBody").finish()
    }
}

impl<D, E> Body for BoxBody<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.inner.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<D, E> Default for BoxBody<D, E>
where
    D: Buf + 'static,
{
    fn default() -> Self {
        BoxBody::new(crate::Empty::new().map_err(|err| match err {}))
    }
}

// === UnsyncBoxBody ===
impl<D, E> UnsyncBoxBody<D, E> {
    /// Create a new `UnsyncBoxBody`.
    pub fn new<B>(body: B) -> Self
    where
        B: Body<Data = D, Error = E> + Send + 'static,
        D: Buf,
    {
        Self {
            inner: ManuallyDrop::new(Box::pin(body)),
            reclaim: None,
        }
    }

    /// The exact allocation layout of the body used by
    /// [`Self::try_new_with_reclaim`]. A zero-sized body requires no allocation.
    pub fn body_layout<B>() -> Layout {
        Layout::new::<B>()
    }

    /// The exact allocation layout of the separately retained reclamation
    /// token used by [`Self::try_new_with_reclaim`]. A zero-sized token requires
    /// no allocation.
    pub fn reclaim_layout<R>() -> Layout {
        Layout::new::<R>()
    }

    /// Fallibly box a body and retain a token until its allocation is freed.
    ///
    /// Callers can admit both exact layouts before construction. Allocation
    /// refusal returns the original body and token without dropping either.
    /// When the result is dropped, the body and its allocation are destroyed,
    /// then the token's storage is freed, and finally the token is dropped.
    /// This order also holds if the body's destructor panics.
    pub fn try_new_with_reclaim<B, R>(body: B, reclaim: R) -> Result<Self, (B, R)>
    where
        B: Body<Data = D, Error = E> + Send + 'static,
        D: Buf,
        R: Send + 'static,
    {
        let body_pointer = match try_allocate::<B>() {
            Some(pointer) => pointer,
            None => return Err((body, reclaim)),
        };
        let reclaim_pointer = match try_allocate::<R>() {
            Some(pointer) => pointer,
            None => {
                let layout = Self::body_layout::<B>();
                if layout.size() != 0 {
                    // SAFETY: the body allocation is still uninitialized and
                    // uniquely owned here; no B destructor must run.
                    unsafe { dealloc(body_pointer.as_ptr().cast::<u8>(), layout) };
                }
                return Err((body, reclaim));
            }
        };
        // SAFETY: both allocations succeeded with the exact native layouts.
        // Nothing fallible remains, and their values are initialized once.
        unsafe {
            body_pointer.as_ptr().write(body);
            reclaim_pointer.as_ptr().write(reclaim);
        }
        // SAFETY: body_pointer is initialized and uniquely owned. The body is
        // pinned only after its final move into the allocation.
        let inner = unsafe { Pin::new_unchecked(Box::from_raw(body_pointer.as_ptr())) };
        Ok(Self {
            inner: ManuallyDrop::new(inner),
            reclaim: Some(ErasedReclaim {
                pointer: reclaim_pointer.cast(),
                release: release_reclaim::<R>,
            }),
        })
    }
}

impl<D, E> fmt::Debug for UnsyncBoxBody<D, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsyncBoxBody").finish()
    }
}

impl<D, E> Body for UnsyncBoxBody<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.inner.as_mut().poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<D, E> Default for UnsyncBoxBody<D, E>
where
    D: Buf + 'static,
{
    fn default() -> Self {
        UnsyncBoxBody::new(crate::Empty::new().map_err(|err| match err {}))
    }
}
