//! # zbus-lockstep
//!
//! Is a collection of helpers for retrieving `DBus` type signatures from XML descriptions.
//! Useful for comparing these with your types' signatures to ensure that they are compatible.
//!
//! It offers functions that retrieve the signature of a method's argument type, of a method's
//! return type, pf a signal's body type or of a property's type from `DBus` XML.
//!
//! These functions require that you provide the file path to the XML file, the interface name,
//! and the interface member wherein the signature resides.
//!
//! Corresponding to each of these functions, macros are provided which do not
//! require you to exactly point out where the signature is found. These will just search
//! by interface member name.
//!
//! The macros assume that the file path to the XML files is either:
//!
//! - `xml` or `XML`, the default path for `DBus` XML files - or is set by the
//! - `LOCKSTEP_XML_PATH`, the env variable that overrides the default.
#![doc(html_root_url = "https://docs.rs/zbus-lockstep/0.7.0")]
#![allow(clippy::missing_errors_doc)]

mod error;
mod macros;

use std::{fmt::Write, io::Read, str::FromStr};

use LockstepError::{ArgumentNotFound, InterfaceNotFound, MemberNotFound, PropertyNotFound};
pub use error::LockstepError;
pub use macros::resolve_xml_path;
use zbus::Signature;
#[cfg(feature = "macros")]
pub use zbus_lockstep_macros::validate;
#[doc(hidden)]
pub use zbus_xml;
use zbus_xml::ArgDirection::{In, Out};

type Result<T> = std::result::Result<T, LockstepError>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum MsgType {
    Method,
    Signal,
    Property,
}

/// Retrieve a signal's body type signature from `DBus` XML.
///
/// If you provide an argument name, then the signature of that argument is returned.
/// If you do not provide an argument name, then the signature of all arguments is returned.
///
/// # Examples
///
/// ```rust
/// # use std::fs::File;
/// # use std::io::{Seek, SeekFrom, Write};
/// # use tempfile::tempfile;
/// use zbus::{Signature, Type, OwnedObjectPath};
/// use zbus_lockstep::get_signal_body_type;
///
/// let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
/// <node xmlns:doc="http://www.freedesktop.org/dbus/1.0/doc.dtd">
/// <interface name="org.freedesktop.bolt1.Manager">
///   <signal name="DeviceAdded">
///    <arg name="device" type="o"/>
///  </signal>
/// </interface>
/// </node>
/// "#;
///
/// let mut xml_file: File = tempfile().unwrap();
/// xml_file.write_all(xml.as_bytes()).unwrap();
/// xml_file.seek(SeekFrom::Start(0)).unwrap();
///
/// #[derive(Debug, PartialEq, Type)]
/// #[zbus(signature = "o")]
/// struct DeviceEvent {
///    device: OwnedObjectPath,
/// }
///
/// let interface_name = "org.freedesktop.bolt1.Manager";
/// let member_name = "DeviceAdded";
///
/// let signature = get_signal_body_type(xml_file, interface_name, member_name, None).unwrap();
///
/// assert_eq!(&signature, DeviceEvent::SIGNATURE);
/// ```
pub fn get_signal_body_type(
    mut xml: impl Read,
    interface_name: &str,
    member_name: &str,
    arg_name: Option<&str>,
) -> Result<Signature> {
    let node = zbus_xml::Node::from_reader(&mut xml)?;

    let interfaces = node.interfaces();
    let interface = interfaces
        .iter()
        .find(|iface| iface.name() == interface_name)
        .ok_or(InterfaceNotFound(interface_name.to_owned()))?;

    let mut signals = interface.signals();
    let signal = signals
        .find(|signal| signal.name() == member_name)
        .ok_or(MemberNotFound(member_name.to_owned()))?;

    let signature: Signature = {
        if let Some(needle_arg_name) = arg_name {
            signal
                .args()
                .iter()
                .find(|signal_arg| signal_arg.name() == Some(needle_arg_name))
                .ok_or(ArgumentNotFound(needle_arg_name.to_owned()))?
                .ty()
                .clone()
        } else {
            let mut combined_sig = String::new();
            for signal_arg in signal.args() {
                write!(combined_sig, "{}", signal_arg.ty())?;
            }
            Signature::from_str(&combined_sig)?
        }
    };

    Ok(signature)
}

/// Retrieve the signature of a property's type from XML.
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use std::io::{Seek, SeekFrom, Write};
/// use tempfile::tempfile;
/// use zbus::Type;
/// use zbus_lockstep::get_property_type;
///
/// #[derive(Debug, PartialEq, Type)]
/// struct InUse(bool);
///
/// let xml = String::from(r#"
/// <node>
/// <interface name="org.freedesktop.GeoClue2.Manager">
///   <property type="b" name="InUse" access="read"/>
/// </interface>
/// </node>
/// "#);
///
/// let mut xml_file: File = tempfile().unwrap();
/// xml_file.write_all(xml.as_bytes()).unwrap();
/// xml_file.seek(SeekFrom::Start(0)).unwrap();
///
/// let interface_name = "org.freedesktop.GeoClue2.Manager";
/// let property_name = "InUse";
///
/// let signature = get_property_type(xml_file, interface_name, property_name).unwrap();
/// assert_eq!(signature, *InUse::SIGNATURE);
/// ```
pub fn get_property_type(
    mut xml: impl Read,
    interface_name: &str,
    property_name: &str,
) -> Result<Signature> {
    let node = zbus_xml::Node::from_reader(&mut xml)?;

    let interfaces = node.interfaces();
    let interface = interfaces
        .iter()
        .find(|iface| iface.name() == interface_name)
        .ok_or(InterfaceNotFound(interface_name.to_string()))?;

    let mut properties = interface.properties();
    let property = properties
        .find(|property| property.name() == property_name)
        .ok_or(PropertyNotFound(property_name.to_owned()))?;

    let signature = property.ty().to_string();
    Ok(Signature::from_str(&signature)?)
}

/// Retrieve the signature of a method's return type from XML.
///
/// If you provide an argument name, then the signature of that argument is returned.
/// If you do not provide an argument name, then the signature of all arguments is returned.
///
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use std::io::{Seek, SeekFrom, Write};
/// use tempfile::tempfile;
/// use zbus::Type;
/// use zbus_lockstep::get_method_return_type;
///
/// #[derive(Debug, PartialEq, Type)]
/// #[repr(u32)]
/// enum Role {
///     Invalid,
///     TitleBar,
///     MenuBar,
///     ScrollBar,
/// }
///
/// let xml = String::from(r#"
/// <node>
/// <interface name="org.a11y.atspi.Accessible">
///    <method name="GetRole">
///       <arg name="role" type="u" direction="out"/>
///   </method>
/// </interface>
/// </node>
/// "#);
///
/// let mut xml_file: File = tempfile().unwrap();
/// xml_file.write_all(xml.as_bytes()).unwrap();
/// xml_file.seek(SeekFrom::Start(0)).unwrap();
///
/// let interface_name = "org.a11y.atspi.Accessible";
/// let member_name = "GetRole";
///
/// let signature = get_method_return_type(xml_file, interface_name, member_name, None).unwrap();
/// assert_eq!(signature, *Role::SIGNATURE);
/// ```
///
/// ## Argument name collisions
///
/// If multiple arguments share the same name within the same direction,
/// the first matching argument's signature is returned.
pub fn get_method_return_type(
    mut xml: impl Read,
    interface_name: &str,
    member_name: &str,
    arg_name: Option<&str>,
) -> Result<Signature> {
    let node = zbus_xml::Node::from_reader(&mut xml)?;

    let interfaces = node.interfaces();
    let interface = interfaces
        .iter()
        .find(|iface| iface.name() == interface_name)
        .ok_or(InterfaceNotFound(interface_name.to_string()))?;

    let mut methods = interface.methods();
    let method = methods
        .find(|method| method.name() == member_name)
        .ok_or(MemberNotFound(member_name.to_string()))?;

    let args = method.args();

    let signature: Signature = {
        if let Some(needle_arg_name) = arg_name {
            args.iter()
                .find(|arg| arg.name() == Some(needle_arg_name) && arg.direction() == Some(Out))
                .ok_or(ArgumentNotFound(needle_arg_name.to_string()))?
                .ty()
                .clone()
        } else {
            let mut combined_sig = String::new();
            for arg in args.iter().filter(|arg| arg.direction() == Some(Out)) {
                write!(combined_sig, "{}", arg.ty())?;
            }
            Signature::from_str(&combined_sig)?
        }
    };

    Ok(signature)
}

/// Retrieve the signature of a method's argument type from XML.
///
/// Useful when one or more arguments, used to call a method, outline a useful type.
///
/// If you provide an argument name, then the signature of that argument is returned.
/// If you do not provide an argument name, then the signature of all arguments to the call is
/// returned.
///
/// # Examples
///
/// ```rust
/// use std::fs::File;
/// use std::collections::HashMap;
/// use std::io::{Seek, SeekFrom, Write};
/// use tempfile::tempfile;
/// use zbus::{Type, Variant};
/// use zbus_lockstep::get_method_args_type;
///
/// let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
/// <node xmlns:doc="http://www.freedesktop.org/dbus/1.0/doc.dtd">
///  <interface name="org.freedesktop.Notifications">
///    <method name="Notify">
///      <arg type="s" name="app_name" direction="in"/>
///      <arg type="u" name="replaces_id" direction="in"/>
///      <arg type="s" name="app_icon" direction="in"/>
///      <arg type="s" name="summary" direction="in"/>
///      <arg type="s" name="body" direction="in"/>
///      <arg type="as" name="actions" direction="in"/>
///      <arg type="a{sv}" name="hints" direction="in"/>
///      <arg type="i" name="expire_timeout" direction="in"/>
///      <arg type="u" name="id" direction="out"/>
///    </method>
///  </interface>
/// </node>
/// "#;
///
/// #[derive(Debug, PartialEq, Type)]
/// struct Notification<'a> {
///    app_name: String,
///    replaces_id: u32,
///    app_icon: String,
///    summary: String,
///    body: String,
///    actions: Vec<String>,
///    hints: HashMap<String, Variant<'a>>,
///    expire_timeout: i32,
/// }
///
/// let mut xml_file = tempfile().unwrap();
/// xml_file.write_all(xml.as_bytes()).unwrap();
/// xml_file.seek(SeekFrom::Start(0)).unwrap();
///
/// let interface_name = "org.freedesktop.Notifications";
/// let member_name = "Notify";
///
/// let signature = get_method_args_type(xml_file, interface_name, member_name, None).unwrap();
/// assert_eq!(&signature, Notification::SIGNATURE);
/// ```
///
/// ## Argument name collisions
/// If multiple arguments share the same name within the same direction,
/// the first matching argument's signature is returned.
pub fn get_method_args_type(
    mut xml: impl Read,
    interface_name: &str,
    member_name: &str,
    arg_name: Option<&str>,
) -> Result<Signature> {
    let node = zbus_xml::Node::from_reader(&mut xml)?;

    let interfaces = node.interfaces();
    let interface = interfaces
        .iter()
        .find(|iface| iface.name() == interface_name)
        .ok_or(InterfaceNotFound(interface_name.to_owned()))?;

    let mut methods = interface.methods();
    let method = methods
        .find(|method| method.name() == member_name)
        .ok_or(MemberNotFound(member_name.to_owned()))?;

    let args = method.args();

    let signature: Signature = if let Some(needle_arg_name) = arg_name {
        args.iter()
            .find(|method_arg| {
                method_arg.name() == Some(needle_arg_name)
                    && method_arg.direction().unwrap_or(In) == In
            })
            .ok_or(ArgumentNotFound(needle_arg_name.to_string()))?
            .ty()
            .clone()
    } else {
        let mut combined_sig = String::new();
        for arg in args
            .iter()
            .filter(|arg| arg.direction().unwrap_or(In) == In)
        {
            write!(combined_sig, "{}", arg.ty())?;
        }
        Signature::from_str(&combined_sig)?
    };

    Ok(signature)
}

#[cfg(test)]
mod test {
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::tempfile;
    use zbus::{OwnedObjectPath, Type};

    use crate::{get_method_args_type, get_method_return_type, get_signal_body_type};

    // Introspection format provides no guarantees that argument names are unique.
    // Even same name, same direction could occur but this would be poor D-Bus API design.
    // zbus-lockstep will filter by appropriate direction and pick the first match.
    //
    // https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format
    #[test]
    fn test_overlapping_names_return_type() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <node>
                <interface name="org.example.Calculator">
                    <method name="Calculate">
                        <arg name="data" type="s" direction="in"/>
                        <arg name="data" type="ai" direction="out"/>
                    </method>
                </interface>
            </node>
        "#;

        let mut xml_file = tempfile().unwrap();
        xml_file.write_all(xml.as_bytes()).unwrap();
        xml_file.seek(SeekFrom::Start(0)).unwrap();

        let sig = get_method_return_type(
            xml_file,
            "org.example.Calculator",
            "Calculate",
            Some("data"),
        )
        .unwrap();

        assert_eq!(&sig.to_string(), "ai");
    }

    #[test]
    fn test_overlapping_names_args_type_swapped() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <node>
                <interface name="org.example.Calculator">
                    <method name="Calculate">
                        <arg name="data" type="ai" direction="out"/>
                        <arg name="data" type="s" direction="in"/>
                    </method>
                </interface>
            </node>
        "#;

        let mut xml_file = tempfile().unwrap();
        xml_file.write_all(xml.as_bytes()).unwrap();
        xml_file.seek(SeekFrom::Start(0)).unwrap();

        let sig = get_method_args_type(
            xml_file,
            "org.example.Calculator",
            "Calculate",
            Some("data"),
        )
        .unwrap();

        assert_eq!(&sig.to_string(), "s");
    }

    #[test]
    fn test_get_signature_of_cache_add_accessible() {
        #[derive(Debug, PartialEq, Type)]
        struct Accessible {
            name: String,
            path: OwnedObjectPath,
        }

        #[derive(Debug, PartialEq, Type)]
        struct CacheItem {
            obj: Accessible,
            application: Accessible,
            parent: Accessible,
            index_in_parent: i32,
            child_count: i32,
            interfaces: Vec<String>,
            name: String,
            role: u32,
            description: String,
            state_set: Vec<u32>,
        }

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <node xmlns:doc="http://www.freedesktop.org/dbus/1.0/doc.dtd">
                <interface name="org.a11y.atspi.Cache">
                    <signal name="AddAccessible">
                        <arg name="nodeAdded" type="((so)(so)(so)iiassusau)"/>
                        <annotation name="org.qtproject.QtDBus.QtTypeName.In0" value="QSpiAccessibleCacheItem"/>
                    </signal>
                </interface>
            </node>
        "#;

        let mut xml_file = tempfile().unwrap();
        xml_file.write_all(xml.as_bytes()).unwrap();
        xml_file.seek(SeekFrom::Start(0)).unwrap();

        let interface_name = "org.a11y.atspi.Cache";
        let member_name = "AddAccessible";

        let signature = get_signal_body_type(xml_file, interface_name, member_name, None).unwrap();
        assert_eq!(signature, *CacheItem::SIGNATURE);
    }
}
