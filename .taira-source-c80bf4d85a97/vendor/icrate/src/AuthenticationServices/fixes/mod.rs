use crate::common::*;

#[cfg(all(
    feature = "apple",
    target_os = "macos",
    feature = "AppKit_NSViewController"
))]
pub type ASViewController = crate::AppKit::NSViewController;
#[cfg(all(
    feature = "apple",
    not(target_os = "macos"),
    feature = "UIKit_UIViewController"
))]
pub type ASViewController = crate::UIKit::UIViewController;
#[cfg(not(any(
    all(
        feature = "apple",
        target_os = "macos",
        feature = "AppKit_NSViewController"
    ),
    all(
        feature = "apple",
        not(target_os = "macos"),
        feature = "UIKit_UIViewController"
    ),
)))]
pub type ASViewController = NSObject;

#[cfg(all(feature = "apple", target_os = "macos", feature = "AppKit_NSWindow"))]
pub type ASPresentationAnchor = crate::AppKit::NSWindow;
#[cfg(all(
    feature = "apple",
    not(target_os = "macos"),
    feature = "UIKit_UIWindow"
))]
pub type ASPresentationAnchor = crate::UIKit::UIWindow;
#[cfg(not(any(
    all(feature = "apple", target_os = "macos", feature = "AppKit_NSWindow"),
    all(
        feature = "apple",
        not(target_os = "macos"),
        feature = "UIKit_UIWindow"
    ),
)))]
pub type ASPresentationAnchor = NSObject;

#[cfg(all(feature = "apple", target_os = "macos", feature = "AppKit_NSImage"))]
pub type ASImage = crate::AppKit::NSImage;
#[cfg(all(feature = "apple", not(target_os = "macos"), feature = "UIKit_UIImage"))]
pub type ASImage = crate::UIKit::UIImage;
#[cfg(not(any(
    all(feature = "apple", target_os = "macos", feature = "AppKit_NSImage"),
    all(feature = "apple", not(target_os = "macos"), feature = "UIKit_UIImage"),
)))]
pub type ASImage = NSObject;

#[cfg(all(
    feature = "AuthenticationServices_ASAuthorizationAppleIDButton",
    feature = "apple",
    target_os = "macos",
    feature = "AppKit_NSControl"
))]
type ASControl = crate::AppKit::NSControl;
#[cfg(all(
    feature = "AuthenticationServices_ASAuthorizationAppleIDButton",
    feature = "apple",
    not(target_os = "macos"),
    feature = "UIKit_UIControl"
))]
type ASControl = crate::UIKit::UIControl;
#[cfg(all(
    feature = "AuthenticationServices_ASAuthorizationAppleIDButton",
    not(any(
        all(feature = "apple", target_os = "macos", feature = "AppKit_NSControl"),
        all(
            feature = "apple",
            not(target_os = "macos"),
            feature = "UIKit_UIControl"
        ),
    ))
))]
type ASControl = NSObject;

#[cfg(all(
    test,
    feature = "apple",
    target_os = "macos",
    feature = "AppKit_NSViewController"
))]
mod tests {
    use super::*;
    use core::any::TypeId;

    #[test]
    fn macos_aliases_resolve_to_appkit_types() {
        assert_eq!(
            TypeId::of::<ASViewController>(),
            TypeId::of::<crate::AppKit::NSViewController>()
        );
        assert_eq!(
            TypeId::of::<ASPresentationAnchor>(),
            TypeId::of::<crate::AppKit::NSWindow>()
        );
        assert_eq!(
            TypeId::of::<ASImage>(),
            TypeId::of::<crate::AppKit::NSImage>()
        );
    }

    #[cfg(feature = "AuthenticationServices_ASAuthorizationAppleIDButton")]
    #[test]
    fn macos_control_alias_matches_nscontrol() {
        assert_eq!(
            TypeId::of::<ASControl>(),
            TypeId::of::<crate::AppKit::NSControl>()
        );
    }
}

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    #[cfg(feature = "AuthenticationServices_ASCredentialProviderViewController")]
    pub struct ASCredentialProviderViewController;

    #[cfg(feature = "AuthenticationServices_ASCredentialProviderViewController")]
    unsafe impl ClassType for ASCredentialProviderViewController {
        type Super = ASViewController;
        type Mutability = InteriorMutable;
    }
);

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    #[cfg(feature = "AuthenticationServices_ASAccountAuthenticationModificationViewController")]
    pub struct ASAccountAuthenticationModificationViewController;

    #[cfg(feature = "AuthenticationServices_ASAccountAuthenticationModificationViewController")]
    unsafe impl ClassType for ASAccountAuthenticationModificationViewController {
        type Super = ASViewController;
        type Mutability = InteriorMutable;
    }
);

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    #[cfg(feature = "AuthenticationServices_ASAuthorizationAppleIDButton")]
    pub struct ASAuthorizationAppleIDButton;

    #[cfg(feature = "AuthenticationServices_ASAuthorizationAppleIDButton")]
    unsafe impl ClassType for ASAuthorizationAppleIDButton {
        type Super = ASControl;
        type Mutability = InteriorMutable;
    }
);
