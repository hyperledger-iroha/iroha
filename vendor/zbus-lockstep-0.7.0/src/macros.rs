#![allow(unused_macros)]
#![allow(dead_code)]
#![allow(unused_imports)]

use std::{fs, path::PathBuf, str::FromStr};

use crate::{LockstepError, Result};

/// Resolve XML path from either:
///
/// This function tries to resolve the XML path from the following sources, in order:
///
/// 1. Environment variable (`LOCKSTEP_XML_PATH`)
/// 2. Provided argument
/// 3. Default location (`xml/`, `XML/`, `../xml` or `../XML` or `<crate_name>/xml` or
///    `<crate_name>/XML`)
///
/// # Example
///
/// ```rust
/// # use zbus_lockstep::resolve_xml_path;
/// # use std::path::PathBuf;
/// # fn main() {
///
/// let xml_path = resolve_xml_path(None).unwrap_or_else(|e| panic!("Failed to resolve XML path: {e}"));
/// assert_eq!(xml_path, PathBuf::from("xml").canonicalize().unwrap());
/// # }
/// ```
pub fn resolve_xml_path(xml: Option<&str>) -> Result<PathBuf> {
    // `LOCKSTEP_XML_PATH` emv variable has precedence over the argument and default paths
    if let Ok(env_path) = std::env::var("LOCKSTEP_XML_PATH") {
        return Ok(PathBuf::from(env_path).canonicalize()?);
    }

    // Provided argument has precedence over the default paths
    if let Some(arg_path) = xml {
        return Ok(PathBuf::from(arg_path).canonicalize()?);
    }

    // Fallback to the default paths:
    let current_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let crate_name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| String::from("unknown"));

    let paths_to_try = [
        current_dir.join("xml"),
        current_dir.join("XML"),
        current_dir.join("../xml"),
        current_dir.join("../XML"),
        current_dir.join(&crate_name).join("xml"),
        current_dir.join(&crate_name).join("XML"),
    ];

    for path in paths_to_try {
        if path.exists() {
            return Ok(path.canonicalize()?);
        }
    }

    Err(LockstepError::PathNotFound(current_dir))
}

/// A generic helper to find the file path and interface name of a member.
#[doc(hidden)]
#[macro_export]
macro_rules! find_definition_in_dbus_xml {
    ($xml_path_buf:expr, $member:expr, $iface:expr, $msg_type:expr) => {{
    use $crate::MsgType;

    let xml_path_buf: std::path::PathBuf = $xml_path_buf;
    let member: &str = $member;
    let iface: Option<String> = $iface;
    let msg_type: MsgType = $msg_type;

    let mut xml_file_path = None;
    let mut interface_name = None;

    let read_dir = std::fs::read_dir(&xml_path_buf).expect("Failed to read XML directory");

    // Walk the XML files in the directory.
    for entry in read_dir {
        let entry = entry.expect("Failed to read entry");

        // Skip directories and non-XML files.
        if entry.path().is_dir() || entry.path().extension().unwrap() != "xml" {
            continue;
        }

        let entry_path = entry.path().clone();
        let file = std::fs::File::open(entry.path()).expect("Failed to open file");
        let node = $crate::zbus_xml::Node::from_reader(file).expect("Failed to parse XML file");

        for interface in node.interfaces() {
            // If called with an `iface` arg, skip he interfaces that do not match.
            if iface.is_some() && interface.name().as_str() != iface.clone().unwrap()  {
                continue;
            }

            match msg_type {
                MsgType::Method => {
                    for dbus_item in interface.methods() {
                        if dbus_item.name() == member {
                            if interface_name.is_some() {
                                panic!(
                                    "Multiple interfaces offer the same {:?} member: {}, please specify the interface name.",
                                    msg_type, member
                                );
                            }
                            interface_name = Some(interface.name().to_string());
                            xml_file_path = Some(entry_path.clone());
                        }
                    }
                }
                MsgType::Signal => {
                    for dbus_item in interface.signals() {
                        if dbus_item.name() == member {
                            if interface_name.is_some() {
                                panic!(
                                    "Multiple interfaces offer the same {:?} member: {}, please specify the interface name.",
                                    msg_type, member
                                );
                            }
                            interface_name = Some(interface.name().to_string());
                            xml_file_path = Some(entry_path.clone());
                        }
                    }
                }
                MsgType::Property => {
                    for dbus_item in interface.properties() {
                        if dbus_item.name() == member {
                            if interface_name.is_some() {
                                panic!(
                                    "Multiple interfaces offer the same {:?} member: {}, please specify the interface name.",
                                    msg_type, member
                                );
                            }
                            interface_name = Some(interface.name().to_string());
                            xml_file_path = Some(entry_path.clone());
                        }
                    }
                }
            };
        }
    }

    // If the interface member was not found, return an error.
    if xml_file_path.is_none() {
        panic!("Member not found in XML files.");
    }

    (xml_file_path.unwrap(), interface_name.unwrap())
    }};
}

/// Retrieve the signature of a method's return type.
///
/// This macro will take a method member name and return the signature of the
/// return type.
///
/// Essentially a wrapper around [`crate::get_method_return_type`],
/// but this macro tries to do its job with less arguments.
///
/// It will search in the XML specification of the method for the return type
/// and return the signature of that type.
///
/// If multiple interfaces offer the same method, you will need to specify the
/// interface name as well.
///
/// This macro can be called with or without the interface name.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use std::str::FromStr;
/// use zbus_lockstep::method_return_signature;
/// use zbus::Signature;
///
/// let sig = method_return_signature!("RequestName");
/// assert_eq!(&sig, &Signature::from_str("u").expect("Valid signature pattern"));
/// ```
/// The macro supports colling arguments with identifiers as well as without.
/// The macro may also be called with an interface name or interface and argument name:
///
/// ```rust
/// # use zbus_lockstep::{method_return_signature};
/// let _sig = method_return_signature!("RequestName", "org.example.Node", "grape");
///
/// // or alternatively
///
/// let _sig = method_return_signature!(member: "RequestName", interface: "org.example.Node", argument: "grape");
/// ```
#[macro_export]
macro_rules! method_return_signature {
    ($member:expr) => {{
        use $crate::MsgType;
        let member = $member;

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, None, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_return_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr) => {
        $crate::method_return_signature!($member)
    };

    ($member:expr, $interface:expr) => {{
        let member = $member;
        use $crate::MsgType;

        let interface = Some($interface.to_string());

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_return_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr, interface: $interface:expr) => {
        $crate::method_return_signature!($member, $interface)
    };

    ($member:expr, $interface:expr, $argument:expr) => {{
        let member = $member;
        use $crate::MsgType;

        let interface = Some($interface.to_string());
        let argument = Some($argument);

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_return_type(file, &interface_name, member, argument)
            .expect("Failed to get method argument(s) type signature")
    }};

    (member: $member:expr, interface: $interface:expr, argument: $argument:expr) => {
        $crate::method_return_signature!($member, $interface, $argument)
    };
}

/// Retrieve the signature of a method's arguments.
///
/// Essentially a wrapper around [`crate::get_method_args_type`],
/// but this macro tries to do its job with less arguments.
///
/// This macro will take a method member name and return the signature of the
/// arguments type.
///
/// It will search in the XML specification of the method for the arguments type
/// and return the signature of that type.
///
/// If multiple interfaces offer the same member, you will need to
/// specify the interface name as well.
///
/// This macro can be called with or without the interface name.
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
/// use zbus_lockstep::method_args_signature;
/// use zbus::Signature;
///
/// let sig = method_args_signature!("RequestName");
/// assert_eq!(&sig, &Signature::from_str("(su)").expect("Valid signature pattern"));
/// ```
/// The macro supports colling arguments with identifiers as well as without.
/// The macro may also be called with an interface name or interface and argument name:
///
/// ```rust
/// # use zbus_lockstep::{method_args_signature};
/// let _sig = method_args_signature!("RequestName", "org.example.Node", "apple");
///
/// // or alternatively
///
/// let _sig = method_args_signature!(member: "RequestName", interface: "org.example.Node", argument: "apple");
/// ```
#[macro_export]
macro_rules! method_args_signature {
    ($member:expr) => {{
        use $crate::MsgType;
        let member = $member;

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, None, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_args_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr) => {
        $crate::method_args_signature!($member)
    };

    ($member:expr, $interface:expr) => {{
        use $crate::MsgType;
        let member = $member;

        let interface = Some($interface.to_string());

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_args_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr, interface: $interface:expr) => {
        $crate::method_args_signature!($member, $interface)
    };

    ($member:expr, $interface:expr, $argument:expr) => {{
        use $crate::MsgType;
        let member = $member;
        let interface = Some($interface.to_string());

        let argument = Some($argument);

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));
        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Method);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_method_args_type(file, &interface_name, member, argument)
            .expect("Failed to get method argument(s) type signature")
    }};

    (member: $member:expr, interface: $interface:expr, argument: $argument:expr) => {
        $crate::method_args_signature!($member, $interface, $argument)
    };
}

/// Retrieve the signature of a signal's body type.
///
/// Essentially a wrapper around [`crate::get_signal_body_type`],
/// but this macro tries to find it with less arguments.
///
/// This macro will take a signal member name and return the signature of the
/// signal body type.
///
/// If multiple interfaces offer the same member, you will need to
/// specify the interface name as well.
///
/// This macro can be called with or without the interface name.
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
/// use zbus_lockstep::signal_body_type_signature;
/// use zbus::Signature;
///
/// let sig = signal_body_type_signature!("AddNode");
/// assert_eq!(&sig, &Signature::from_str("(so)").expect("Valid signature pattern"));
/// ```
/// The macro supports colling arguments with identifiers as well as without.
/// The macro may also be called with an interface name or interface and argument name:
///
/// ```rust
/// # use zbus_lockstep::{signal_body_type_signature};
/// let _sig = signal_body_type_signature!("Alert", "org.example.Node", "color");
///
/// // or alternatively
///
/// let _sig = signal_body_type_signature!(member: "Alert", interface: "org.example.Node", argument: "color");
/// ```
#[macro_export]
macro_rules! signal_body_type_signature {
    ($member:expr) => {{
        use $crate::MsgType;
        let member = $member;

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, None, MsgType::Signal);

        let file = std::fs::File::open(file_path).expect("Failed to open file");

        $crate::get_signal_body_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr) => {
        $crate::signal_body_type_signature!($member)
    };

    ($member:expr, $interface:expr) => {{
        use $crate::MsgType;
        let member = $member;
        let interface = Some($interface.to_string());

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Signal);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_signal_body_type(file, &interface_name, member, None)
            .expect("Failed to get method arguments type signature")
    }};

    (member: $member:expr, interface: $interface:expr) => {
        $crate::signal_body_type_signature!($member, $interface)
    };

    ($member:expr, $interface:expr, $argument:expr) => {{
        use $crate::MsgType;
        let member = $member;
        let interface = Some($interface.to_string());

        let argument = Some($argument);

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Signal);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_signal_body_type(file, &interface_name, member, argument)
            .expect("Failed to get method argument(s) type signature")
    }};

    (member: $member:expr, interface: $interface:expr, argument: $argument:expr) => {
        $crate::signal_body_type_signature!($member, $interface, $argument)
    };
}

/// Retrieve the signature of a property's type.
///
/// Essentially a wrapper around [`crate::get_property_type`],
/// but this macro tries to do with less arguments.
///
/// This macro will take a property name and return the signature of the
/// property's type.
///
/// If multiple interfaces offer the same member, you will need to
/// specify the interface name as well.
///
/// This macro can be called with or without the interface name.
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
/// use zbus_lockstep::property_type_signature;
/// use zbus::Signature;
///
/// let sig = property_type_signature!("Features");
/// assert_eq!(&sig, &Signature::from_str("as").expect("Valid signature pattern"));
/// ```
/// The member name and/or interface name can be used tp identify the arguments:
///
/// ```rust
/// # use zbus_lockstep::{property_type_signature};
/// let _sig = property_type_signature!(member: "Features", interface: "org.example.Node");
/// ```
#[macro_export]
macro_rules! property_type_signature {
    ($member:expr) => {{
        use $crate::MsgType;
        let member = $member;

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, None, MsgType::Property);

        let file = std::fs::File::open(file_path).expect("Failed to open file");

        $crate::get_property_type(file, &interface_name, member)
            .expect("Failed to get property type signature")
    }};

    (member: $member:expr) => {
        $crate::property_type_signature!($member)
    };

    ($member:expr, $interface:expr) => {{
        use $crate::MsgType;
        let member = $member;
        let interface = Some($interface.to_string());

        // Looking for default path or path specified by environment variable.
        let xml_path = $crate::resolve_xml_path(None)
            .unwrap_or_else(|err| panic!("Failed to resolve XML path: {err}"));

        // Find the definition of the method in the XML specification.
        let (file_path, interface_name) =
            $crate::find_definition_in_dbus_xml!(xml_path, member, interface, MsgType::Property);

        let file = std::fs::File::open(file_path).expect("Failed to open file");
        $crate::get_property_type(file, &interface_name, member)
            .expect("Failed to get property type signature")
    }};

    (member: $member:expr, interface: $interface:expr) => {
        $crate::property_type_signature!($member, $interface)
    };
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use zbus::Signature;

    use crate::signal_body_type_signature;

    #[test]
    fn test_signal_body_signature_macro() {
        // Path to XML files can be set by setting environment variable `LOCKSTEP_XML_PATH`
        // However, `resolve_xml_path` can find the `xml` in parent.
        //
        // Note that setting the environment variable with std::env::set_var is not recommended:
        // https://doc.rust-lang.org/stable/std/env/fn.set_var.html#safety

        let sig = crate::signal_body_type_signature!("AddNode");
        assert_eq!(
            &sig,
            &zbus::Signature::from_str("(so)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_signal_body_signature_macro_with_identifier() {
        let sig = crate::signal_body_type_signature!(member: "AddNode");
        assert_eq!(
            sig,
            Signature::from_str("(so)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_signal_body_signature_macro_with_interface() {
        let sig = crate::signal_body_type_signature!("AddNode", "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("(so)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_signal_body_signature_macro_with_interface_and_identifiers() {
        let sig =
            crate::signal_body_type_signature!(member: "AddNode", interface: "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("(so)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_signal_body_signature_macro_with_argument_and_interface() {
        let sig = crate::signal_body_type_signature!("Alert", "org.example.Node", "volume");
        assert_eq!(
            sig,
            Signature::from_str("d").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_signal_body_signature_macro_with_argument_and_identifiers_and_interface() {
        let sig = crate::signal_body_type_signature!(
            member: "Alert",
            interface: "org.example.Node",
            argument: "urgent"
        );
        assert_eq!(
            sig,
            Signature::from_str("b").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro() {
        let sig = crate::method_args_signature!("RequestName");
        assert_eq!(
            sig,
            Signature::from_str("(su)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro_with_identifier() {
        let sig = crate::method_args_signature!(member: "RequestName");
        assert_eq!(
            sig,
            Signature::from_str("(su)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro_with_interface() {
        let sig = crate::method_args_signature!("RequestName", "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("(su)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro_with_interface_and_identifiers() {
        let sig =
            crate::method_args_signature!(member: "RequestName", interface: "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("(su)").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro_with_argument_and_interface() {
        let sig = crate::method_args_signature!("RequestName", "org.example.Node", "apple");
        assert_eq!(
            sig,
            Signature::from_str("s").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_args_signature_macro_with_argument_and_identifiers_and_interface() {
        let sig = crate::method_args_signature!(
            member: "RequestName",
            interface: "org.example.Node",
            argument: "orange"
        );
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro() {
        let sig = crate::method_return_signature!("RequestName");
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Valid signatuee pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro_with_identifier() {
        let sig = crate::method_return_signature!(member: "RequestName");
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro_with_interface() {
        let sig = crate::method_return_signature!("RequestName", "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Valid signature pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro_with_interface_and_identifiers() {
        let sig =
            crate::method_return_signature!(member: "RequestName", interface: "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro_with_argument_and_interface() {
        let sig = crate::method_return_signature!("RequestName", "org.example.Node", "grape");
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_method_return_signature_macro_with_argument_and_identifiers_and_interface() {
        let sig = crate::method_return_signature!(
            member: "RequestName",
            interface: "org.example.Node",
            argument: "grape"
        );
        assert_eq!(
            sig,
            Signature::from_str("u").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_property_type_signature_macro() {
        let sig = crate::property_type_signature!("Features");
        assert_eq!(
            sig,
            Signature::from_str("as").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_property_type_signature_macro_with_identifier() {
        let sig = crate::property_type_signature!(member: "Features");
        assert_eq!(
            sig,
            Signature::from_str("as").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_property_type_signature_macro_with_interface() {
        let sig = crate::property_type_signature!("Features", "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("as").expect("Vlaid signature pattern")
        );
    }

    #[test]
    fn test_property_type_signature_macro_with_interface_and_identifiers() {
        let sig =
            crate::property_type_signature!(member: "Features", interface: "org.example.Node");
        assert_eq!(
            sig,
            Signature::from_str("as").expect("Vlaid signature pattern")
        );
    }
}
