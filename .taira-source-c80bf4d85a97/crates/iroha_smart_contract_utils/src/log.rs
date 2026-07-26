//! IVM logging utilities

pub use iroha_data_model::Level;

/// Log `obj` with desired log level.
///
/// Prints the output along with its level to stderr.
#[doc(hidden)]
pub fn __log<T: std::string::ToString + ?Sized>(log_level: Level, obj: &T) {
    eprintln!("{}: {}", log_level, obj.to_string());
}

/// Construct a new event
#[macro_export]
macro_rules! event {
    ($log_level:path, $msg:expr) => {
        $crate::log::__log($log_level, $msg)
    };
}

/// Construct an event at the trace level.
#[macro_export]
macro_rules! trace {
    ($msg:expr) => {
        $crate::event!($crate::log::Level::TRACE, $msg)
    };
}

/// Construct an event at the debug level.
#[macro_export]
macro_rules! debug {
    ($msg:expr) => {
        $crate::event!($crate::log::Level::DEBUG, $msg)
    };
}

/// Construct an event at the info level.
#[macro_export]
macro_rules! info {
    ($msg:expr) => {
        $crate::event!($crate::log::Level::INFO, $msg)
    };
}

/// Construct an event at the warn level.
#[macro_export]
macro_rules! warn {
    ($msg:expr) => {
        $crate::event!($crate::log::Level::WARN, $msg)
    };
}

/// Construct an event at the error level.
#[macro_export]
macro_rules! error {
    ($msg:expr) => {
        $crate::event!($crate::log::Level::ERROR, $msg)
    };
}

#[cfg(test)]
mod tests {
    const LOG_MESSAGE: &str = "log_message";

    #[test]
    fn log_call() {
        crate::warn!(LOG_MESSAGE);
    }
}
