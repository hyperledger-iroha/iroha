//! IVM debugging utilities

use core::fmt::Debug;

/// Print `obj` in debug representation.
///
/// When running inside the IVM or on the host, prints to stderr.
/// Does nothing unless `debug` feature is enabled.
#[doc(hidden)]
#[allow(unused_variables)]
pub fn __dbg<T: Debug + ?Sized>(obj: &T) {
    #[cfg(feature = "debug")]
    {
        eprintln!("dbg: {obj:?}");
    }
}

/// Print `obj` in debug representation. Does nothing unless `debug` feature is enabled.
/// When running inside the IVM, prints to host's stdout.
/// When running outside of the IVM, always prints the output to stderr
#[macro_export]
macro_rules! dbg {
    () => {
        #[cfg(feature = "debug")]
        $crate::__dbg(concat!("[{}:{}:{}]", core::file!(), core::line!(), core::column!()));
    };
    ($val:expr $(,)?) => {{
        #[cfg(feature = "debug")]
        match $val {
            tmp => {
                let location = concat!("[{}:{}:{}]", core::file!(), core::line!(), core::column!());
                let location = std::format!("{location} {} = {tmp:#?}", stringify!($val));
                $crate::__dbg(&location);
            }
        }
    }};
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

/// Print `obj` in debug representation. Does nothing unless `debug` feature is enabled.
/// When running inside the IVM, prints to host's stderr.
/// When running outside of the IVM, always prints the output to stderr
#[macro_export]
macro_rules! dbg_panic {
    () => {
        dbg!();
        panic!();
    };
    ($val:expr $(,)?) => {{
        match $val {
            tmp => {
                dbg!(tmp);
                panic!("{tmp:?}");
            }
        }
    }};
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

/// Extension implemented for `Result` and `Option` to provide unwrapping with error message,
/// cause basic `unwrap()` does not print error due to specific panic handling in the IVM runtime.
///
/// Expands to just `unwrap()` if `debug` feature is not specified
pub trait DebugUnwrapExt {
    /// Type of the value that is returned in success
    type Output;

    /// Just like `unwrap()` but prints error message before panic
    fn dbg_unwrap(self) -> Self::Output;
}

impl<T, E: Debug> DebugUnwrapExt for Result<T, E> {
    type Output = T;

    fn dbg_unwrap(self) -> Self::Output {
        #[cfg(not(feature = "debug"))]
        return self.unwrap();

        #[cfg(feature = "debug")]
        match self {
            Ok(res) => res,
            Err(err) => panic!("{err:?}"),
        }
    }
}

impl<T> DebugUnwrapExt for Option<T> {
    type Output = T;

    fn dbg_unwrap(self) -> Self::Output {
        #[cfg(not(feature = "debug"))]
        return self.unwrap();

        #[cfg(feature = "debug")]
        match self {
            Some(res) => res,
            None => panic!("unwrapped a None"),
        }
    }
}

/// Extension implemented for `Result` and `Option` to provide expecting with error message
/// while still printing debug information when `debug` feature is enabled.
pub trait DebugExpectExt {
    /// Type of the value returned on success
    type Output;

    /// Like `expect` but prints error before panic when debugging is enabled.
    fn dbg_expect(self, msg: &str) -> Self::Output;
}

impl<T, E: Debug> DebugExpectExt for Result<T, E> {
    type Output = T;

    fn dbg_expect(self, msg: &str) -> Self::Output {
        #[cfg(not(feature = "debug"))]
        return self.expect(msg);

        #[cfg(feature = "debug")]
        match self {
            Ok(res) => res,
            Err(err) => panic!("{msg}: {err:?}"),
        }
    }
}

impl<T> DebugExpectExt for Option<T> {
    type Output = T;

    fn dbg_expect(self, msg: &str) -> Self::Output {
        #[cfg(not(feature = "debug"))]
        return self.expect(msg);

        #[cfg(feature = "debug")]
        match self {
            Some(res) => res,
            None => panic!("{msg}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dbg_call() {
        dbg!("dbg_message");
    }
}
