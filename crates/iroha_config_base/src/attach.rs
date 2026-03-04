//! Various attachments for [`error_stack::Report::attach`] API.

use std::{
    any::TypeId,
    collections::HashSet,
    fmt::{Debug, Display, Formatter},
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use derive_more::Display;
use error_stack::{Report, fmt::ColorMode};

const ANSI_RESET: &str = "\u{1b}[0m";
const ANSI_BOLD_CYAN: &str = "\u{1b}[1;36m";
const ANSI_ITALIC: &str = "\u{1b}[3m";

fn colorize_attachment_message(message: &str, mode: ColorMode) -> String {
    match mode {
        ColorMode::None => message.to_string(),
        ColorMode::Color | ColorMode::Emphasis => {
            let (label, rest) = message
                .find(':')
                .map_or((message, ""), |idx| message.split_at(idx + 1));

            let styled_label = match mode {
                ColorMode::Color => format!("{ANSI_BOLD_CYAN}{label}{ANSI_RESET}"),
                ColorMode::Emphasis => format!("{ANSI_ITALIC}{label}{ANSI_RESET}"),
                ColorMode::None => unreachable!(),
            };

            format!("{styled_label}{rest}")
        }
    }
}

fn ensure_display_hook<T>()
where
    T: Display + Send + Sync + 'static,
{
    static INSTALLED: OnceLock<Mutex<HashSet<TypeId>>> = OnceLock::new();

    let registry = INSTALLED.get_or_init(|| Mutex::new(HashSet::new()));
    let type_id = TypeId::of::<T>();

    {
        let mut guard = registry
            .lock()
            .expect("hook installation registry is poisoned");
        if !guard.insert(type_id) {
            return;
        }
    }

    Report::install_debug_hook::<T>(|value, context| {
        let message = format!("{value}");
        let formatted = colorize_attachment_message(&message, context.color_mode());
        context.push_body(formatted);
    });
}

use crate::{ParameterId, ParameterOrigin};

/// Attach a file path
#[derive(Display, Debug)]
#[display("file path: {}", path.display())]
pub struct FilePath {
    path: PathBuf,
}

impl FilePath {
    /// Construct a [`FilePath`] attachment describing the offending path.
    pub fn new(path: PathBuf) -> Self {
        ensure_display_hook::<Self>();
        Self { path }
    }
}

/// Attach an actual value
#[derive(Display, Debug)]
#[display("actual value: {value}")]
pub struct ActualValue<T>
where
    T: Display + Debug,
{
    value: T,
}

impl<T> ActualValue<T>
where
    T: Display + Debug + Send + Sync + 'static,
{
    /// Construct an [`ActualValue`] attachment capturing the encountered value.
    pub fn new(value: T) -> Self {
        ensure_display_hook::<Self>();
        Self { value }
    }
}

/// Attach an expectation
#[derive(Display, Debug)]
#[display("expected: {message}")]
pub struct Expected<T>
where
    T: Display + Debug,
{
    message: T,
}

impl<T> Expected<T>
where
    T: Display + Debug + Send + Sync + 'static,
{
    /// Construct an [`Expected`] attachment describing the expected shape.
    pub fn new(message: T) -> Self {
        ensure_display_hook::<Self>();
        Self { message }
    }
}

/// Attach a chain of extensions (see [`crate::util::ExtendsPaths`])
#[derive(Display, Debug)]
#[display("extending ({depth}): `{}` -> `{}`", from.display(), to.display())]
pub struct ExtendsChain {
    from: PathBuf,
    to: PathBuf,
    depth: u8,
}

impl ExtendsChain {
    /// Describe the inheritance chain between configuration files.
    pub fn new(from: PathBuf, to: PathBuf, depth: u8) -> Self {
        ensure_display_hook::<Self>();
        Self { from, to, depth }
    }
}

/// Attach a missing parameter identifier.
#[derive(Display, Debug)]
#[display("missing parameter: `{id}`")]
pub struct MissingParameter {
    id: ParameterId,
}

impl MissingParameter {
    /// Construct a [`MissingParameter`] attachment.
    pub fn new(id: ParameterId) -> Self {
        ensure_display_hook::<Self>();
        Self { id }
    }
}

/// Attach an unknown parameter identifier.
#[derive(Display, Debug)]
#[display("unknown parameter: `{id}`")]
pub struct UnknownParameter {
    id: ParameterId,
}

impl UnknownParameter {
    /// Construct an [`UnknownParameter`] attachment.
    pub fn new(id: ParameterId) -> Self {
        ensure_display_hook::<Self>();
        Self { id }
    }
}

/// Attach an environment key-value entry
#[derive(Display, Debug)]
#[display("value: {var}={value}")]
pub struct EnvValue {
    var: String,
    value: String,
}

impl EnvValue {
    /// Capture the raw environment variable value used during configuration.
    pub fn new(var: String, value: String) -> Self {
        ensure_display_hook::<Self>();
        Self { var, value }
    }
}

/// Attach a raw configuration value rendered as a string.
#[derive(Display, Debug)]
#[display("actual value: {value}")]
pub struct ConfigValue {
    value: String,
}

impl ConfigValue {
    /// Capture a configuration value for diagnostics.
    pub fn new(value: impl Into<String>) -> Self {
        ensure_display_hook::<Self>();
        Self {
            value: value.into(),
        }
    }
}

/// Attach config value and its origin.
///
/// Usually constructed via [`crate::WithOrigin::into_attachment`].
///
/// To support displaying values which don't implement [Display], it uses formats mechanism.
/// For example:
///
/// - For [Path], use [`ConfigValueAndOrigin::display_path`]
/// - For [Debug], use [`ConfigValueAndOrigin::display_as_debug`]
///
/// Example usage with a path:
///
/// ```
/// use std::path::PathBuf;
/// use error_stack::Report;
/// use iroha_config_base::{ParameterOrigin, WithOrigin};
///
/// let value = PathBuf::from("/path/to/somewhere");
/// let attachment = WithOrigin::new(
///     value,
///     ParameterOrigin::file(
///         ["a", "b"].into(),
///         PathBuf::from("/root/iroha/config.toml")
///     )
/// )
/// .into_attachment()
/// .display_path();
///
/// assert_eq!(
///     format!("{attachment}"),
///     "config origin: parameter `a.b` with value `/path/to/somewhere` in file `/root/iroha/config.toml`"
/// );
///
/// let _report = Report::new(std::io::Error::other("test")).attach(attachment);
/// ```
pub struct ConfigValueAndOrigin<T, Format = FormatDisplay<T>> {
    value: T,
    origin: ParameterOrigin,
    _f: PhantomData<Format>,
}

impl<T, F> Debug for ConfigValueAndOrigin<T, F>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigValueAndOrigin")
            .field("value", &self.value)
            .field("origin", &self.origin)
            .finish()
    }
}

impl<T, F> ConfigValueAndOrigin<T, F> {
    fn new_internal(value: T, origin: ParameterOrigin) -> Self {
        Self {
            value,
            origin,
            _f: PhantomData,
        }
    }
}

impl<T> ConfigValueAndOrigin<T> {
    /// Constructor
    pub fn new(value: T, origin: ParameterOrigin) -> Self {
        ConfigValueAndOrigin::new_internal(value, origin)
    }
}

impl<T: AsRef<Path>> ConfigValueAndOrigin<T> {
    /// Switch to [`FormatPath`]
    pub fn display_path(self) -> ConfigValueAndOrigin<T, FormatPath<T>> {
        ConfigValueAndOrigin::new_internal(self.value, self.origin)
    }
}

impl<T: Debug> ConfigValueAndOrigin<T> {
    /// Switch to [`FormatDebug`]
    pub fn display_as_debug(self) -> ConfigValueAndOrigin<T, FormatDebug<T>> {
        ConfigValueAndOrigin::new_internal(self.value, self.origin)
    }
}

/// Workaround that [`ConfigValueAndOrigin`] uses to display a value that doesn't
/// implement [`Display`] directly using some format, e.g. [`FormatPath`].
pub trait DisplayProxy {
    /// Associated type for which the implementor is proxying [`Display`] implementation.
    type Base: ?Sized;

    /// Similar to [`Display::fmt`], but uses an associated type instead of `self`.
    #[allow(clippy::missing_errors_doc)]
    fn fmt(value: &Self::Base, f: &mut Formatter<'_>) -> std::fmt::Result;
}

/// Indicates formating of a value that implements [`Display`].
pub struct FormatDisplay<T>(PhantomData<T>);

impl<T: Display> DisplayProxy for FormatDisplay<T> {
    type Base = T;

    fn fmt(value: &Self::Base, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{value}")
    }
}

/// Indicates formatting of a [`Path`] using [`Path::display`].
#[allow(missing_copy_implementations)]
pub struct FormatPath<T>(PhantomData<T>);

impl<T: AsRef<Path>> DisplayProxy for FormatPath<T> {
    type Base = T;

    fn fmt(value: &Self::Base, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", value.as_ref().display())
    }
}

/// Indicates formatting using [`Debug`].
pub struct FormatDebug<T>(PhantomData<T>);

impl<T: Debug> DisplayProxy for FormatDebug<T> {
    type Base = T;

    fn fmt(value: &Self::Base, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{value:?}")
    }
}

struct DisplayWithProxy<'a, T, F>(&'a T, PhantomData<F>);

impl<T, F> Display for DisplayWithProxy<'_, T, F>
where
    F: DisplayProxy<Base = T>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <F as DisplayProxy>::fmt(self.0, f)
    }
}

impl<T, F> Display for ConfigValueAndOrigin<T, F>
where
    F: DisplayProxy<Base = T>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { origin, value, .. } = &self;
        let value = DisplayWithProxy(value, PhantomData::<F>);

        write!(f, "config origin: ")?;

        match origin {
            ParameterOrigin::File { id, path } => write!(
                f,
                "parameter `{id}` with value `{value}` in file `{}`",
                path.display()
            ),
            ParameterOrigin::Env { id, var } => write!(
                f,
                "parameter `{id}` with value `{value}` set from environment variable `{var}`"
            ),
            ParameterOrigin::Default { id } => {
                write!(f, "parameter `{id}` with default value `{value}`")
            }
            ParameterOrigin::Custom { message } => write!(f, "{message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Error, ErrorKind},
        sync::{Mutex, OnceLock},
    };

    use error_stack::{Report, fmt::ColorMode};

    use super::*;

    static COLOR_MODE_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    use toml::Value;

    use crate::ParameterId;

    fn reset_color_mode(mode: ColorMode) {
        Report::set_color_mode(mode);
    }

    #[test]
    fn file_path_hook_colors_label() {
        let _guard = COLOR_MODE_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("color mode guard poisoned");
        reset_color_mode(ColorMode::Color);

        let report = Report::new(Error::from(ErrorKind::Other))
            .attach(FilePath::new(PathBuf::from("/tmp/config")));
        let rendered = format!("{report:?}");

        reset_color_mode(ColorMode::Emphasis);

        assert!(rendered.contains("\u{1b}[1;36mfile path:"));
        assert!(rendered.contains("/tmp/config"));
    }

    #[test]
    fn actual_value_hook_emphasizes_label() {
        let _guard = COLOR_MODE_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("color mode guard poisoned");
        reset_color_mode(ColorMode::Emphasis);

        let value = Value::String("./other.toml".to_owned());
        let report =
            Report::new(Error::from(ErrorKind::InvalidInput)).attach(ActualValue::new(value));
        let rendered = format!("{report:?}");
        reset_color_mode(ColorMode::Emphasis);

        assert!(rendered.contains("\u{1b}[3mactual value:"));
        assert!(rendered.contains("./other.toml"));
    }

    #[test]
    fn missing_parameter_attachment_formats() {
        let attachment = MissingParameter::new(ParameterId::from(["network", "port"]));
        assert_eq!(format!("{attachment}"), "missing parameter: `network.port`");
    }

    #[test]
    fn missing_parameter_hook_renders_in_report() {
        let _guard = COLOR_MODE_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("color mode guard poisoned");
        reset_color_mode(ColorMode::None);

        let report = Report::new(Error::from(ErrorKind::Other))
            .attach(MissingParameter::new(ParameterId::from(["foo"])));
        let rendered = format!("{report:?}");

        assert!(rendered.contains("missing parameter: `foo`"));
    }

    #[test]
    fn unknown_parameter_attachment_formats() {
        let attachment = UnknownParameter::new(ParameterId::from(["database", "dsn"]));
        assert_eq!(format!("{attachment}"), "unknown parameter: `database.dsn`");
    }

    #[test]
    fn unknown_parameter_hook_renders_in_report() {
        let _guard = COLOR_MODE_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("color mode guard poisoned");
        reset_color_mode(ColorMode::None);

        let report = Report::new(Error::from(ErrorKind::Other))
            .attach(UnknownParameter::new(ParameterId::from(["bar"])));
        let rendered = format!("{report:?}");

        assert!(rendered.contains("unknown parameter: `bar`"));
    }
}
