use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug)]
pub enum LockstepError {
    ArgumentNotFound(String),
    InterfaceNotFound(String),
    MemberNotFound(String),
    PropertyNotFound(String),
    PathNotFound(PathBuf),
    Env(std::env::VarError),
    Fmt(std::fmt::Error),
    Io(std::io::Error),
    Xml(zbus_xml::Error),
    Zbus(zbus::Error),
}

impl std::error::Error for LockstepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LockstepError::Env(e) => Some(e),
            LockstepError::Fmt(e) => Some(e),
            LockstepError::Io(e) => Some(e),
            LockstepError::Xml(e) => Some(e),
            LockstepError::Zbus(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for LockstepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockstepError::ArgumentNotFound(name) => write!(f, "Argument \"{name}\" not found."),
            LockstepError::InterfaceNotFound(name) => write!(f, "Interface \"{name}\" not found."),
            LockstepError::MemberNotFound(name) => write!(f, "Member \"{name}\" not found."),
            LockstepError::PropertyNotFound(name) => write!(f, "Property \"{name}\" not found."),
            LockstepError::PathNotFound(path) => write!(
                f,
                "No XML path provided and default XML path not found. Searched directory: \"{}\"",
                path.display()
            ),
            LockstepError::Env(e) => write!(f, "Environment variable error: {e}"),
            LockstepError::Fmt(e) => write!(f, "Formatting error: {e}"),
            LockstepError::Io(e) => write!(f, "IO error: {e}"),
            LockstepError::Xml(e) => write!(f, "XML error: {e}"),
            LockstepError::Zbus(e) => write!(f, "Zbus signature error: {e}"),
        }
    }
}

impl From<std::io::Error> for LockstepError {
    fn from(e: std::io::Error) -> Self {
        LockstepError::Io(e)
    }
}

impl From<zbus_xml::Error> for LockstepError {
    fn from(e: zbus_xml::Error) -> Self {
        LockstepError::Xml(e)
    }
}

impl From<zbus::Error> for LockstepError {
    fn from(e: zbus::Error) -> Self {
        LockstepError::Zbus(e)
    }
}

impl From<zbus::wire::signature::Error> for LockstepError {
    fn from(e: zbus::wire::signature::Error) -> Self {
        // Under-the-hood `zbus` converts this to `zbus::Error::SignatureParse(e)`
        LockstepError::Zbus(zbus::Error::from(e))
    }
}

impl From<std::env::VarError> for LockstepError {
    fn from(e: std::env::VarError) -> Self {
        LockstepError::Env(e)
    }
}

impl From<std::fmt::Error> for LockstepError {
    fn from(e: std::fmt::Error) -> Self {
        LockstepError::Fmt(e)
    }
}

// `LockstepError` wraps upstream errors (e.g. `io::Error`) that are `!UnwindSafe`
// because they may carry a `Box<dyn Error>`, which the compiler conservatively
// assumes could hide interior mutability. This propagates and strips the auto
// traits from `LockstepError`.
//
// These impls are sound: `LockstepError` is passive, owned error data with no
// interior mutability (`Cell`/`RefCell`) and no handles to shared mutable state.
// It is only ever read (e.g. via `Display`), never mutated, so a panic can never
// leave it holding a broken invariant that could be observed across a
// `catch_unwind` boundary.
impl std::panic::UnwindSafe for LockstepError {}
impl std::panic::RefUnwindSafe for LockstepError {}
