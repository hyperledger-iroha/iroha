//! CUDA driver bindings resolved at runtime, with no CUDA link-time dependency.

macro_rules! cuda_result_enum {
    ($(#[$attribute:meta])* pub enum $name:ident { $($variant:ident = $value:literal),* $(,)? }) => {
        $(#[$attribute])*
        pub enum $name { $($variant = $value),* }

        impl $name {
            // Newer drivers may return status codes absent from these bindings.
            // Read C integers first, then construct only valid Rust discriminants.
            pub(crate) const fn from_driver_code(value: i32) -> Self {
                match value {
                    $($value => Self::$variant,)*
                    _ => Self::CUDA_ERROR_UNKNOWN,
                }
            }
        }
    };
}

macro_rules! cuda_driver_functions {
    (pub fn cuGetErrorString($error:ident: $error_ty:ty, $out:ident: $out_ty:ty) -> CUresult;) => {
        cuda_driver_functions!(@impl cuGetErrorString($error: $error_ty, $out: $out_ty);
            |load_error| {
                // SAFETY: the caller supplies the declared writable output pointer.
                unsafe { crate::driver::loader_error_text($error, $out, load_error, false) }
            }
        );
    };
    (pub fn cuGetErrorName($error:ident: $error_ty:ty, $out:ident: $out_ty:ty) -> CUresult;) => {
        cuda_driver_functions!(@impl cuGetErrorName($error: $error_ty, $out: $out_ty);
            |load_error| {
                // SAFETY: the caller supplies the declared writable output pointer.
                unsafe { crate::driver::loader_error_text($error, $out, load_error, true) }
            }
        );
    };
    (pub fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> CUresult;) => {
        cuda_driver_functions!(@impl $name($($arg: $ty),*); |error| error);
    };
    (@impl $name:ident($($arg:ident: $ty:ty),*); $fallback:expr) => {
        #[doc = concat!("Calls the runtime CUDA driver symbol `", stringify!($name), "`.")]
        ///
        /// # Safety
        /// The caller must satisfy the CUDA Driver API requirements for every
        /// pointer, device handle, context and stream passed to this function.
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> CUresult {
            type Function = unsafe extern "C" fn($($ty),*) -> i32;
            static FUNCTION: std::sync::OnceLock<Function> = std::sync::OnceLock::new();
            // A missing driver may become available after the discovery cooldown.
            // Keep only resolved pointers process-lived; failed loads must retry.
            let function = crate::driver::load_or_cached(&FUNCTION, || {
                // SAFETY: arguments retain the generated C signature. CUresult
                // has the C int representation, normalized after the call below.
                unsafe {
                    crate::driver::resolve::<Function>(
                        concat!(stringify!($name), "\0").as_bytes(),
                    )
                }
            });
            match function {
                // SAFETY: the caller upholds the documented driver preconditions.
                Ok(function) => CUresult::from_driver_code(unsafe { function($($arg),*) }),
                Err(error) => ($fallback)(error),
            }
        }
    };
}

// Generated driver declarations retain NVIDIA's names and bindgen layout tests.
#[allow(warnings)]
mod cuda;
mod driver;

pub use cuda::*;
