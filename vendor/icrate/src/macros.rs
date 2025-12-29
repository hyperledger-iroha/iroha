#![allow(unused_macros)]

macro_rules! impl_encode {
    ($name:ident = $encoding:expr;) => {
        #[cfg(feature = "objective-c")]
        unsafe impl objc2::Encode for $name {
            const ENCODING: objc2::Encoding = $encoding;
        }

        #[cfg(feature = "objective-c")]
        unsafe impl objc2::RefEncode for $name {
            const ENCODING_REF: objc2::Encoding =
                objc2::Encoding::Pointer(&<Self as objc2::Encode>::ENCODING);
        }
    };
}

macro_rules! extern_struct {
    (
        #[encoding_name($encoding_name:literal)]
        $(#[$m:meta])*
        $v:vis struct $name:ident {
            $($field_v:vis $field:ident: $ty:ty),* $(,)?
        }
    ) => {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        $(#[$m])*
        $v struct $name {
            $($field_v $field: $ty,)*
        }

        impl_encode! {
            $name = objc2::Encoding::Struct(
                $encoding_name,
                &[$(<$ty as objc2::Encode>::ENCODING),*],
            );
        }
    };
    (
        $(#[$m:meta])*
        $v:vis struct $name:ident {
            $($field_v:vis $field:ident: $ty:ty),* $(,)?
        }
    ) => {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        $(#[$m])*
        $v struct $name {
            $($field_v $field: $ty,)*
        }

        impl_encode! {
            $name = objc2::Encoding::Struct(
                stringify!($name),
                &[$(<$ty as objc2::Encode>::ENCODING),*],
            );
        }
    };
}

macro_rules! extern_enum {
    (
        #[underlying($ty:ty)]
        $(#[$m:meta])*
        $v:vis enum __anonymous__ {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        extern_enum! {
            @__inner
            @($v)
            @($ty)
            @($(
                $(#[$field_m])*
                $field = $value,
            )*)
        }
    };
    (
        #[underlying($ty:ty)]
        $(#[$m:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        // TODO: Improve type-safety
        $(#[$m])*
        $v type $name = $ty;

        extern_enum! {
            @__inner
            @($v)
            @($name)
            @($(
                $(#[$field_m])*
                $field = $value,
            )*)
        }
    };

    // tt-munch each field
    (
        @__inner
        @($v:vis)
        @($ty:ty)
        @()
    ) => {
        // Base case
    };
    (
        @__inner
        @($v:vis)
        @($ty:ty)
        @(
            $(#[$field_m:meta])*
            $field:ident = $value:expr,

            $($rest:tt)*
        )
    ) => {
        $(#[$field_m])*
        $v const $field: $ty = $value;

        extern_enum! {
            @__inner
            @($v)
            @($ty)
            @($($rest)*)
        }
    };
}

/// Corresponds to `NS_ENUM`
macro_rules! ns_enum {
    (
        #[underlying($ty:ty)]
        $(#[$m:meta])*
        $v:vis enum $($name:ident)? {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        extern_enum! {
            #[underlying($ty)]
            $(#[$m])*
            $v enum $($name)? {
                $(
                    $(#[$field_m])*
                    $field = $value
                ),*
            }
        }
    };
}

/// Corresponds to `NS_OPTIONS`
macro_rules! ns_options {
    (
        #[underlying($ty:ty)]
        $(#[$m:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        $(#[$m])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
        $v struct $name(pub $ty);

        impl $name {
            $(
                $(#[$field_m])*
                pub const $field: Self = Self($value);
            )*

            /// Empty option set.
            pub const fn empty() -> Self {
                Self(0)
            }

            /// Returns `true` if all bits in `other` are set in `self`.
            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut first = true;
                $(
                    if self.contains(Self::$field) {
                        if !first {
                            f.write_str(" | ")?;
                        }
                        f.write_str(stringify!($field))?;
                        first = false;
                    }
                )*
                if first {
                    f.write_str("empty")
                } else {
                    Ok(())
                }
            }
        }

        impl ::core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl ::core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl ::core::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl ::core::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl ::core::ops::Not for $name {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl From<$ty> for $name {
            fn from(value: $ty) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $ty {
            fn from(value: $name) -> $ty {
                value.0
            }
        }
    };
}

/// Corresponds to `NS_CLOSED_ENUM`
macro_rules! ns_closed_enum {
    (
        #[underlying(NSUInteger)]
        $(#[$m:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        // TODO: Handle this differently
        extern_enum! {
            #[underlying(NSUInteger)]
            $(#[$m])*
            $v enum $name {
                $(
                    $(#[$field_m])*
                    $field = $value
                ),*
            }
        }
    };
    (
        #[underlying(NSInteger)]
        $(#[$m:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        // TODO: Handle this differently
        extern_enum! {
            #[underlying(NSInteger)]
            $(#[$m])*
            $v enum $name {
                $(
                    $(#[$field_m])*
                    $field = $value
                ),*
            }
        }
    };
}

/// Corresponds to `NS_ERROR_ENUM`
macro_rules! ns_error_enum {
    (
        #[underlying(NSInteger)]
        $(#[$m:meta])*
        $v:vis enum $($name:ident)? {
            $(
                $(#[$field_m:meta])*
                $field:ident = $value:expr
            ),* $(,)?
        }
    ) => {
        // TODO: Handle this differently
        extern_enum! {
            #[underlying(NSInteger)]
            $(#[$m])*
            $v enum $($name)? {
                $(
                    $(#[$field_m])*
                    $field = $value
                ),*
            }
        }
    };
}

macro_rules! typed_enum {
    ($v:vis type $name:ident = $ty:ty $(;)?) => {
        // TODO
        pub type $name = $ty;
    };
}

macro_rules! typed_extensible_enum {
    ($v:vis type $name:ident = $ty:ty $(;)?) => {
        // TODO
        pub type $name = $ty;
    };
}

macro_rules! extern_static {
    ($name:ident: $ty:ty) => {
        extern "C" {
            pub static $name: $ty;
        }
    };
    // Floats in statics are broken
    ($name:ident: NSAppKitVersion = $($value:tt)*) => {
        pub static $name: NSAppKitVersion = $($value)* as _;
    };
    ($name:ident: NSLayoutPriority = $($value:tt)*) => {
        pub static $name: NSLayoutPriority = $($value)* as _;
    };
    ($name:ident: NSStackViewVisibilityPriority = $($value:tt)*) => {
        pub static $name: NSStackViewVisibilityPriority = $($value)* as _;
    };
    ($name:ident: NSTouchBarItemPriority = $($value:tt)*) => {
        pub static $name: NSTouchBarItemPriority = $($value)* as _;
    };
    ($name:ident: MKFeatureDisplayPriority = $($value:tt)*) => {
        pub static $name: MKFeatureDisplayPriority = $($value)* as _;
    };
    ($name:ident: MKAnnotationViewZPriority = $($value:tt)*) => {
        pub static $name: MKAnnotationViewZPriority = $($value)* as _;
    };
    ($name:ident: $ty:ty = $value:expr) => {
        pub static $name: $ty = $value;
    };
}

macro_rules! extern_fn {
    (
        $(#[$m:meta])*
        $v:vis unsafe fn $name:ident($($args:tt)*) $(-> $res:ty)?;
    ) => {
        $(#[$m])*
        extern "C" {
            $v fn $name($($args)*) $(-> $res)?;
        }
    };
    (
        $(#[$m:meta])*
        $v:vis fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $res:ty)?;
    ) => {
        #[inline]
        $(#[$m])*
        $v extern "C" fn $name($($arg: $arg_ty),*) $(-> $res)? {
            extern "C" {
                fn $name($($arg: $arg_ty),*) $(-> $res)?;
            }
            unsafe {
                $name($($arg),*)
            }
        }
    };
}

macro_rules! inline_fn {
    (
        $(#[$m:meta])*
        $v:vis unsafe fn $name:ident($($args:tt)*) $(-> $res:ty)? $body:block
    ) => {
        // TODO
    };
}
