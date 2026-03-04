//! Extensive export tests for `iroha_ffi`: enums, structs, options, slices, and more.
#![allow(unsafe_code, clippy::pedantic)]

use std::{
    collections::BTreeMap,
    mem::{ManuallyDrop, MaybeUninit},
    string::String,
};

use iroha_ffi::{
    FfiConvert, FfiOutPtrRead, FfiReturn, FfiTuple1, FfiTuple2, FfiType, FromFfiReturn,
    IntoFfiReturn, LocalRef, ReprC, Result, ffi_export,
    ir::Ir,
    option::{self, OptionIr},
    repr_c::{
        COutPtr, COutPtrRead, COutPtrWrite, CType, CTypeConvert, CWrapperType, Cloned, NonLocal,
        read_non_local, write_non_local,
    },
    slice::OutBoxedSlice,
};

iroha_ffi::handles! {OpaqueStruct}
iroha_ffi::def_ffi_fns! { dealloc }

/// Simple trait used in tests to exercise associated types and methods across FFI.
pub trait Target {
    /// Associated type carried by the implementer.
    type Target;

    /// Convert `self` into the associated type.
    fn target(self) -> Self::Target;
}

/// Test wrapper for a name string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FfiType)]
pub struct Name(String);
/// Test wrapper for a value string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FfiType)]
pub struct Value(String);

/// Opaque structure
#[derive(Debug, Clone, PartialEq, Eq, Default, FfiType)]
pub struct OpaqueStruct {
    name: Option<Name>,
    tokens: Vec<Value>,
    params: BTreeMap<Name, Value>,
}

/// Fieldless enum
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FfiType)]
pub enum FieldlessEnum {
    /// Variant A
    A,
    /// Variant B
    B,
    /// Variant C
    C,
}

/// Single-variant transparent enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FfiType)]
#[repr(transparent)]
pub enum TransparentFieldlessEnum {
    /// Single variant
    A,
}

/// Data-carrying enum
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(variant_size_differences)]
pub enum DataCarryingEnum {
    /// Struct variant
    A(OpaqueStruct),
    /// Numeric variant
    B(u32),
    /// Value payload variant
    C(Value),
    /// Empty variant
    D,
}

#[repr(C)]
#[derive(Clone, Copy)]
/// ReprC container used to bridge `DataCarryingEnum` across FFI tests.
pub struct __iroha_ffi__ReprCDataCarryingEnum {
    /// Variant discriminant encoded as a compact integer.
    pub tag: u8,
    /// Union carrying the variant payload.
    pub payload: __iroha_ffi__DataCarryingEnumPayload,
}

#[repr(C)]
#[derive(Clone, Copy)]
/// Payload union accompanying `__iroha_ffi__ReprCDataCarryingEnum`.
pub union __iroha_ffi__DataCarryingEnumPayload {
    /// Payload for `DataCarryingEnum::A`.
    pub variant_a: ManuallyDrop<<OpaqueStruct as FfiType>::ReprC>,
    /// Payload for `DataCarryingEnum::B`.
    pub variant_b: u32,
    /// Payload for `DataCarryingEnum::C`.
    pub variant_c: ManuallyDrop<<Value as FfiType>::ReprC>,
    /// Sentinel for the `DataCarryingEnum::D` variant.
    pub variant_d: (),
}

unsafe impl ReprC for __iroha_ffi__ReprCDataCarryingEnum {}
unsafe impl ReprC for __iroha_ffi__DataCarryingEnumPayload {}

#[derive(Default)]
/// Conversion state for `DataCarryingEnum` when marshalling into `ReprC`.
pub struct DataCarryingEnumRustStore<'itm> {
    a: <OpaqueStruct as FfiConvert<'itm, <OpaqueStruct as FfiType>::ReprC>>::RustStore,
    c: <Value as FfiConvert<'itm, <Value as FfiType>::ReprC>>::RustStore,
}

#[derive(Default)]
/// Conversion state for `DataCarryingEnum` when decoding from `ReprC`.
pub struct DataCarryingEnumFfiStore<'itm> {
    a: <OpaqueStruct as FfiConvert<'itm, <OpaqueStruct as FfiType>::ReprC>>::FfiStore,
    c: <Value as FfiConvert<'itm, <Value as FfiType>::ReprC>>::FfiStore,
}

impl Ir for DataCarryingEnum {
    type Type = Self;
}

impl OptionIr for DataCarryingEnum {
    type Type = option::WithoutNiche;
}

impl Cloned for DataCarryingEnum {}

unsafe impl NonLocal<Self> for DataCarryingEnum {}

impl CType<DataCarryingEnum> for DataCarryingEnum {
    type ReprC = __iroha_ffi__ReprCDataCarryingEnum;
}

impl<'itm> CTypeConvert<'itm, DataCarryingEnum, __iroha_ffi__ReprCDataCarryingEnum>
    for DataCarryingEnum
{
    type RustStore = DataCarryingEnumRustStore<'itm>;
    type FfiStore = DataCarryingEnumFfiStore<'itm>;

    fn into_repr_c(self, store: &'itm mut Self::RustStore) -> __iroha_ffi__ReprCDataCarryingEnum {
        match self {
            DataCarryingEnum::A(value) => {
                let repr = value.into_ffi(&mut store.a);
                __iroha_ffi__ReprCDataCarryingEnum {
                    tag: 0,
                    payload: __iroha_ffi__DataCarryingEnumPayload {
                        variant_a: ManuallyDrop::new(repr),
                    },
                }
            }
            DataCarryingEnum::B(value) => __iroha_ffi__ReprCDataCarryingEnum {
                tag: 1,
                payload: __iroha_ffi__DataCarryingEnumPayload { variant_b: value },
            },
            DataCarryingEnum::C(value) => {
                let repr = value.into_ffi(&mut store.c);
                __iroha_ffi__ReprCDataCarryingEnum {
                    tag: 2,
                    payload: __iroha_ffi__DataCarryingEnumPayload {
                        variant_c: ManuallyDrop::new(repr),
                    },
                }
            }
            DataCarryingEnum::D => __iroha_ffi__ReprCDataCarryingEnum {
                tag: 3,
                payload: __iroha_ffi__DataCarryingEnumPayload { variant_d: () },
            },
        }
    }

    unsafe fn try_from_repr_c(
        value: __iroha_ffi__ReprCDataCarryingEnum,
        store: &'itm mut Self::FfiStore,
    ) -> Result<Self> {
        match value.tag {
            0 => {
                let payload = ManuallyDrop::into_inner(unsafe { value.payload.variant_a });
                let extracted = unsafe {
                    <OpaqueStruct as FfiConvert<
                        'itm,
                        <OpaqueStruct as FfiType>::ReprC,
                    >>::try_from_ffi(payload, &mut store.a)?
                };
                Ok(DataCarryingEnum::A(extracted))
            }
            1 => Ok(DataCarryingEnum::B(unsafe { value.payload.variant_b })),
            2 => {
                let payload = ManuallyDrop::into_inner(unsafe { value.payload.variant_c });
                let extracted = unsafe {
                    <Value as FfiConvert<'itm, <Value as FfiType>::ReprC>>::try_from_ffi(
                        payload,
                        &mut store.c,
                    )?
                };
                Ok(DataCarryingEnum::C(extracted))
            }
            3 => Ok(DataCarryingEnum::D),
            _ => Err(FfiReturn::TrapRepresentation),
        }
    }
}

impl CWrapperType<DataCarryingEnum> for DataCarryingEnum {
    type InputType = DataCarryingEnum;
    type ReturnType = DataCarryingEnum;
}

impl COutPtr<DataCarryingEnum> for DataCarryingEnum {
    type OutPtr = __iroha_ffi__ReprCDataCarryingEnum;
}

impl COutPtrWrite<DataCarryingEnum> for DataCarryingEnum {
    unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
        unsafe { write_non_local::<_, DataCarryingEnum>(self, out_ptr) };
    }
}

impl COutPtrRead<DataCarryingEnum> for DataCarryingEnum {
    unsafe fn try_read_out(out_ptr: Self::OutPtr) -> Result<Self> {
        unsafe { read_non_local::<DataCarryingEnum, DataCarryingEnum>(out_ptr) }
    }
}

/// `ReprC` struct
#[derive(Clone, Copy, PartialEq, Eq, FfiType)]
#[repr(C)]
pub struct RobustReprCStruct<T, U> {
    a: u8,
    b: T,
    c: U,
    d: core::mem::ManuallyDrop<i16>,
}

#[ffi_export]
impl OpaqueStruct {
    /// New
    pub fn new(name: Name) -> Self {
        Self {
            name: Some(name),
            tokens: Vec::new(),
            params: BTreeMap::default(),
        }
    }

    /// Consume self
    pub fn consume_self(self) {}

    /// With tokens
    #[must_use]
    pub fn with_tokens(mut self, tokens: impl IntoIterator<Item = impl Into<Value>>) -> Self {
        self.tokens = tokens.into_iter().map(Into::into).collect();
        self
    }

    /// With params
    #[must_use]
    // Note: `-> OpaqueStruct` used instead of `-> Self` to showcase that such signature supported by `#[ffi_export]`
    pub fn with_params(mut self, params: impl IntoIterator<Item = (Name, Value)>) -> OpaqueStruct {
        self.params = params.into_iter().collect();
        self
    }

    /// Get param
    pub fn get_param(&self, name: &Name) -> Option<&Value> {
        self.params.get(name)
    }

    /// Params
    pub fn params(&self) -> impl ExactSizeIterator<Item = (&Name, &Value)> {
        self.params.iter()
    }

    /// Remove parameter
    pub fn remove_param(&mut self, param: &Name) -> Option<Value> {
        self.params.remove(param)
    }

    /// Fallible int output
    pub fn fallible_int_output(flag: bool) -> std::result::Result<u32, &'static str> {
        if flag { Ok(42) } else { Err("fail") }
    }

    /// Fallible empty tuple output
    pub fn fallible_empty_tuple_output(flag: bool) -> std::result::Result<(), &'static str> {
        if flag { Ok(()) } else { Err("fail") }
    }
}

#[ffi_export]
/// Take and return boxed slice
pub fn freestanding_with_boxed_slice(item: Box<[u8]>) -> Box<[u8]> {
    item
}

#[ffi_export]
/// Take and return byte
pub fn freestanding_with_option(item: Option<u8>) -> Option<u8> {
    item
}

#[ffi_export]
/// Take and return byte
pub fn freestanding_with_option_with_niche_ref(item: &Option<bool>) -> &Option<bool> {
    item
}

#[ffi_export]
/// Take and return byte
pub fn freestanding_with_option_without_niche_ref(item: &Option<u8>) -> &Option<u8> {
    item
}

#[ffi_export]
/// Take and return byte
pub fn freestanding_with_primitive(byte: u8) -> u8 {
    byte
}

/// Take and return fieldless enum
#[ffi_export]
pub fn freestanding_with_fieldless_enum(enum_: FieldlessEnum) -> FieldlessEnum {
    enum_
}

/// Return data-carrying enum
#[ffi_export]
pub fn freestanding_with_data_carrying_enum(enum_: DataCarryingEnum) -> DataCarryingEnum {
    enum_
}

/// Return array as pointer
#[ffi_export]
pub fn freestanding_with_array(arr: [u8; 1]) -> [u8; 1] {
    arr
}

/// Take and return array reference
#[ffi_export]
pub fn freestanding_with_array_ref(arr: &[u8; 1]) -> &[u8; 1] {
    arr
}

/// Return array wrapped in a tuple
#[ffi_export]
pub fn freestanding_with_array_in_struct(arr: ([u8; 1],)) -> ([u8; 1],) {
    arr
}

/// Return a `#[repr(C)]` union
#[ffi_export]
pub fn freestanding_with_repr_c_struct(
    struct_: RobustReprCStruct<u32, i16>,
) -> RobustReprCStruct<u32, i16> {
    struct_
}

/// Return array wrapped in a tuple
#[ffi_export]
#[allow(clippy::vec_box)]
pub fn get_vec_of_boxed_opaques() -> Vec<Box<OpaqueStruct>> {
    vec![Box::new(get_new_struct())]
}

/// Take and return array
#[ffi_export]
pub fn take_and_return_array_of_opaques(a: [OpaqueStruct; 2]) -> [OpaqueStruct; 2] {
    a
}

/// Receive nested vector
#[ffi_export]
pub fn freestanding_with_nested_vec(_vec: Vec<Vec<Vec<u8>>>) {}

/// Take `&mut String`
#[ffi_export]
#[cfg(feature = "non_robust_ref_mut")]
pub fn take_non_robust_ref_mut(val: &mut str) -> &mut str {
    val
}

/// Accepts a vector reference; used to ensure FFI signatures compile.
#[ffi_export]
/// Accepts a vector reference; used to ensure FFI signatures compile.
pub fn take_vec_ref(a: &Vec<u8>) {
    assert_eq!(a, &vec![1, 2])
}

/// Returns the same tuple reference; used to test tuple passing across FFI.
/// Returns the same tuple reference; used to test tuple passing across FFI.
#[ffi_export]
pub fn take_tuple_ref(a: &(u8, u8)) -> &(u8, u8) {
    a
}

#[ffi_export]
impl Target for OpaqueStruct {
    type Target = Option<Name>;

    fn target(self) -> <Self as Target>::Target {
        self.name
    }
}

/// Returns the first element reference from a slice.
/// Returns the first element reference from a slice.
#[ffi_export]
pub fn reference_from_slice(a: &[u8]) -> &u8 {
    &a[0]
}

fn get_default_params() -> [(Name, Value); 2] {
    [
        (Name(String::from("Nomen")), Value(String::from("Omen"))),
        (Name(String::from("Nomen2")), Value(String::from("Omen2"))),
    ]
}

fn get_new_struct() -> OpaqueStruct {
    let name = Name(String::from("X"));

    unsafe {
        let mut ffi_struct = MaybeUninit::new(core::ptr::null_mut());

        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__new(FfiConvert::into_ffi(name, &mut ()), ffi_struct.as_mut_ptr())
        );

        let ffi_struct = ffi_struct.assume_init();
        FfiConvert::try_from_ffi(ffi_struct, &mut ()).unwrap()
    }
}

fn get_new_struct_with_params() -> OpaqueStruct {
    let ffi_struct = get_new_struct();
    let params = get_default_params().to_vec();

    let mut output = MaybeUninit::new(core::ptr::null_mut());

    let mut store = Default::default();
    let params_ffi = params.into_ffi(&mut store);
    assert_eq!(FfiReturn::Ok, unsafe {
        OpaqueStruct__with_params(
            FfiConvert::into_ffi(ffi_struct, &mut ()),
            params_ffi,
            output.as_mut_ptr(),
        )
    });

    unsafe { FfiConvert::try_from_ffi(output.assume_init(), &mut ()).expect("valid") }
}

#[test]
#[cfg(feature = "non_robust_ref_mut")]
fn non_robust_ref_mut() {
    use iroha_ffi::slice::RefMutSlice;

    let mut owned = "queen".to_owned();
    let ffi_struct: &mut str = owned.as_mut();
    let mut output = MaybeUninit::new(RefMutSlice::from_raw_parts_mut(core::ptr::null_mut(), 0));
    let ffi_type: RefMutSlice<u8> = FfiConvert::into_ffi(ffi_struct, &mut ());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __take_non_robust_ref_mut(ffi_type, output.as_mut_ptr())
        );

        let output: &mut str = FfiOutPtrRead::try_read_out(output.assume_init()).unwrap();
        assert_eq!(output, owned.as_mut());
    }
}

#[test]
fn constructor() {
    let ffi_struct = get_new_struct();
    assert_eq!(Some(Name(String::from('X'))), ffi_struct.name);
    assert!(ffi_struct.params.is_empty());
}

#[test]
fn builder_method() {
    let ffi_struct = get_new_struct_with_params();

    assert_eq!(2, ffi_struct.params.len());
    assert_eq!(
        ffi_struct.params,
        get_default_params().into_iter().collect()
    );
}

#[test]
fn consume_self() {
    let ffi_struct = get_new_struct();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__consume_self(ffi_struct.into_ffi(&mut ()).cast())
        );
    }
}

#[test]
fn into_iter_item_impl_into() {
    let tokens = vec![
        Value(String::from("My omen")),
        Value(String::from("Your omen")),
    ];

    let mut ffi_struct = get_new_struct();
    let mut tokens_store = Box::default();
    let tokens_ffi = tokens.clone().into_ffi(&mut tokens_store);

    let mut output = MaybeUninit::new(core::ptr::null_mut());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__with_tokens(
                FfiConvert::into_ffi(ffi_struct, &mut ()),
                tokens_ffi,
                output.as_mut_ptr()
            )
        );

        ffi_struct = FfiConvert::try_from_ffi(output.assume_init(), &mut ()).expect("valid");

        assert_eq!(2, ffi_struct.tokens.len());
        assert_eq!(ffi_struct.tokens, tokens);
    }
}

#[test]
fn mutate_opaque() {
    let param_name = Name(String::from("Nomen"));
    let mut ffi_struct = get_new_struct_with_params();
    let mut removed = MaybeUninit::new(core::ptr::null_mut());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__remove_param(
                FfiConvert::into_ffi(&mut ffi_struct, &mut ()),
                &param_name,
                removed.as_mut_ptr(),
            )
        );

        let removed = removed.assume_init();
        let removed = Option::try_from_ffi(removed, &mut ()).unwrap();
        assert_eq!(Some(Value(String::from("Omen"))), removed);
        assert!(!ffi_struct.params.contains_key(&param_name));
    }
}

#[test]
fn return_option() {
    let ffi_struct = get_new_struct_with_params();

    let mut param1 = MaybeUninit::new(core::ptr::null());
    let mut param2 = MaybeUninit::new(core::ptr::null());

    let name1 = Name(String::from("Non"));
    assert_eq!(FfiReturn::Ok, unsafe {
        OpaqueStruct__get_param(
            FfiConvert::into_ffi(&ffi_struct, &mut ()),
            &name1,
            param1.as_mut_ptr(),
        )
    });
    let param1 = unsafe { param1.assume_init() };
    assert!(param1.is_null());
    let param1: Option<&Value> = unsafe { FfiConvert::try_from_ffi(param1, &mut ()).unwrap() };
    assert!(param1.is_none());

    let name2 = Name(String::from("Nomen"));
    assert_eq!(FfiReturn::Ok, unsafe {
        OpaqueStruct__get_param(
            FfiConvert::into_ffi(&ffi_struct, &mut ()),
            &name2,
            param2.as_mut_ptr(),
        )
    });

    unsafe {
        let param2 = param2.assume_init();
        assert!(!param2.is_null());
        let param2: Option<&Value> = FfiConvert::try_from_ffi(param2, &mut ()).unwrap();
        assert_eq!(Some(&Value(String::from("Omen"))), param2);
    }
}

#[test]
fn take_and_return_boxed_slice() {
    let input: Box<[u8]> = [12u8, 42u8].into();
    let mut output = MaybeUninit::new(OutBoxedSlice::from_raw_parts(core::ptr::null_mut(), 0));
    let mut in_store = Default::default();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_boxed_slice(
                FfiConvert::into_ffi(input, &mut in_store),
                output.as_mut_ptr()
            )
        );

        let output = output.assume_init();
        assert_eq!(output.len(), 2);
        let boxed_slice: Box<[u8]> = iroha_ffi::FfiOutPtrRead::try_read_out(output).expect("Valid");
        assert_eq!(boxed_slice, [12u8, 42u8].into());
    }
}

#[test]
fn take_and_return_option_without_niche() {
    let input: Option<u8> = Some(42);
    let mut output = MaybeUninit::uninit();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_option(FfiConvert::into_ffi(input, &mut ()), output.as_mut_ptr())
        );

        let output = output.assume_init();
        assert_eq!(input, FfiOutPtrRead::try_read_out(output).expect("Valid"));
    }
}

#[test]
fn take_and_return_option_with_niche_ref() {
    let input = Some(true);
    let mut output = MaybeUninit::new(0);
    let mut in_store = Default::default();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_option_with_niche_ref(
                FfiConvert::into_ffi(&input, &mut in_store),
                output.as_mut_ptr()
            )
        );

        let output = output.assume_init();
        let restored: LocalRef<'_, Option<bool>> =
            iroha_ffi::FfiOutPtrRead::try_read_out(output).expect("Valid");
        assert_eq!(input, *restored);
    }
}

#[test]
fn take_and_return_option_without_niche_ref() {
    let input = Some(42u8);
    let mut output = MaybeUninit::uninit();
    let mut in_store = Default::default();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_option_without_niche_ref(
                FfiConvert::into_ffi(&input, &mut in_store),
                output.as_mut_ptr()
            )
        );

        let output = output.assume_init();
        let restored: LocalRef<'_, Option<u8>> =
            iroha_ffi::FfiOutPtrRead::try_read_out(output).expect("Valid");
        assert_eq!(input, *restored);
    }
}

#[test]
fn return_iterator() {
    let ffi_struct = get_new_struct_with_params();
    let mut out_params = MaybeUninit::new(OutBoxedSlice::from_raw_parts(core::ptr::null_mut(), 0));

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__params(
                FfiConvert::into_ffi(&ffi_struct, &mut ()),
                out_params.as_mut_ptr()
            )
        );

        let out_params = out_params.assume_init();
        assert_eq!(out_params.len(), 2);
        let vec: Vec<(&Name, &Value)> =
            iroha_ffi::FfiOutPtrRead::try_read_out(out_params).expect("Valid");

        let default_params = get_default_params();
        assert_eq!((&default_params[0].0, &default_params[0].1), vec[0]);
        assert_eq!((&default_params[1].0, &default_params[1].1), vec[1]);
    }
}

#[test]
fn return_result() {
    let mut output = MaybeUninit::new(0);

    unsafe {
        assert_eq!(
            FfiReturn::ExecutionFail,
            OpaqueStruct__fallible_int_output(false.into_ffi(&mut ()), output.as_mut_ptr())
        );
        assert_eq!(0, output.assume_init());
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__fallible_int_output(true.into_ffi(&mut ()), output.as_mut_ptr())
        );
        assert_eq!(42, output.assume_init());
    }
}

#[test]
fn return_empty_tuple_result() {
    unsafe {
        assert_eq!(
            FfiReturn::ExecutionFail,
            OpaqueStruct__fallible_empty_tuple_output(false.into_ffi(&mut ()))
        );
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__fallible_empty_tuple_output(true.into_ffi(&mut ()))
        );
    }
}

#[test]
fn ffi_return_conversion_helpers() {
    assert_eq!(
        IntoFfiReturn::into_ffi_return("oops"),
        FfiReturn::ExecutionFail
    );
    assert_eq!(
        <&'static str as FromFfiReturn>::from_ffi_return(FfiReturn::ExecutionFail),
        ""
    );
    assert_eq!(
        <String as FromFfiReturn>::from_ffi_return(FfiReturn::UnknownHandle),
        "UnknownHandle"
    );
}

#[test]
fn array_to_pointer() {
    let array = [1_u8];
    let mut store = Option::default();
    let ptr: *mut [u8; 1] = array.into_ffi(&mut store);
    let mut output = MaybeUninit::new([0_u8]);

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_array(ptr, output.as_mut_ptr())
        );

        assert_eq!(
            [1_u8],
            <[u8; 1]>::try_from_ffi(output.assume_init(), &mut ()).unwrap()
        );
    }
}

#[test]
fn take_and_return_array_ref() {
    let array = [1_u8];
    let ptr: *const [u8; 1] = (&array).into_ffi(&mut ());
    let mut output = MaybeUninit::new(core::ptr::null());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_array_ref(ptr, output.as_mut_ptr())
        );

        assert_eq!(
            &[1_u8; 1],
            <&[u8; 1]>::try_from_ffi(output.assume_init(), &mut ()).unwrap()
        );
    }
}

#[test]
fn array_in_struct() {
    let array = ([1_u8],);
    let ffi_arr: FfiTuple1<[u8; 1]> = array.into_ffi(&mut ((),));
    let mut output = MaybeUninit::new(FfiTuple1([0; 1]));

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_array_in_struct(ffi_arr, output.as_mut_ptr())
        );

        assert_eq!(
            ([1_u8],),
            <([u8; 1],)>::try_from_ffi(output.assume_init(), &mut ((),)).unwrap()
        );
    }
}

#[test]
fn repr_c_struct() {
    let struct_ = RobustReprCStruct {
        a: 42,
        b: 7,
        c: 12,
        d: core::mem::ManuallyDrop::new(12),
    };
    let mut output = MaybeUninit::new(RobustReprCStruct {
        a: u8::MAX,
        b: u32::MAX,
        c: i16::MAX,
        d: core::mem::ManuallyDrop::new(-1),
    });

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_repr_c_struct(struct_, output.as_mut_ptr())
        );

        assert!(output.assume_init() == struct_);
    }
}

#[test]
fn primitive_conversion() {
    let byte: u8 = 1;
    let mut output = MaybeUninit::new(0);

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_primitive(byte.into_ffi(&mut ()), output.as_mut_ptr())
        );

        assert_eq!(1, output.assume_init());
    }
}

#[test]
fn fieldless_enum_conversion() {
    let fieldless_enum = FieldlessEnum::A;
    let mut output = MaybeUninit::new(2);

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_fieldless_enum(
                fieldless_enum.into_ffi(&mut ()),
                output.as_mut_ptr()
            )
        );

        let ret_val = FfiOutPtrRead::try_read_out(output.assume_init());
        assert_eq!(FieldlessEnum::A, ret_val.expect("Conversion failed"));
    }
}

#[cfg(feature = "ivm")]
#[test]
fn primitive_conversion_failed() {
    let byte: u32 = u32::MAX;
    let mut output = MaybeUninit::new(0);

    unsafe {
        assert_eq!(
            FfiReturn::ConversionFailed,
            __freestanding_with_primitive(byte, output.as_mut_ptr())
        );

        assert_eq!(0, output.assume_init());
    }
}

#[test]
fn data_carrying_enum_conversion() {
    let data_carrying_enum = DataCarryingEnum::A(get_new_struct());
    let mut output = MaybeUninit::<__iroha_ffi__ReprCDataCarryingEnum>::uninit();

    unsafe {
        let mut store = Default::default();
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_data_carrying_enum(
                data_carrying_enum.clone().into_ffi(&mut store),
                output.as_mut_ptr()
            )
        );

        let ret_val = FfiOutPtrRead::try_read_out(output.assume_init());
        assert_eq!(data_carrying_enum, ret_val.expect("Conversion failed"));
    }
}

#[test]
fn data_carrying_enum_value_payload_conversion() {
    let data_carrying_enum = DataCarryingEnum::C(Value(String::from("payload")));
    let mut output = MaybeUninit::<__iroha_ffi__ReprCDataCarryingEnum>::uninit();

    unsafe {
        let mut store = Default::default();
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_data_carrying_enum(
                data_carrying_enum.clone().into_ffi(&mut store),
                output.as_mut_ptr()
            )
        );

        let ret_val = FfiOutPtrRead::try_read_out(output.assume_init());
        assert_eq!(data_carrying_enum, ret_val.expect("Conversion failed"));
    }
}

#[test]
fn invoke_trait_method() {
    let ffi_struct = get_new_struct_with_params();
    let mut output = MaybeUninit::<*mut Name>::new(core::ptr::null_mut());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            OpaqueStruct__Target__target(
                FfiConvert::into_ffi(ffi_struct, &mut ()),
                output.as_mut_ptr()
            )
        );
        let name = FfiConvert::try_from_ffi(output.assume_init(), &mut ()).unwrap();
        assert_eq!(Name(String::from("X")), name);
    }
}

#[test]
fn nested_vec() {
    let vec: Vec<Vec<Vec<u8>>> = vec![];

    unsafe {
        let mut store = Default::default();
        assert_eq!(
            FfiReturn::Ok,
            __freestanding_with_nested_vec(vec.into_ffi(&mut store))
        );
    }
}

#[test]
fn return_vec_of_boxed_opaques() {
    let mut output = MaybeUninit::new(OutBoxedSlice::from_raw_parts(core::ptr::null_mut(), 0));

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __get_vec_of_boxed_opaques(output.as_mut_ptr())
        );
        let output = output.assume_init();
        assert_eq!(output.len(), 1);
        let vec: Vec<Box<OpaqueStruct>> =
            iroha_ffi::FfiOutPtrRead::try_read_out(output).expect("Valid");
        assert_eq!(Box::new(get_new_struct()), vec[0]);
    }
}

#[test]
fn array_of_opaques() {
    let input = [OpaqueStruct::default(), OpaqueStruct::default()];
    let mut output = MaybeUninit::new([core::ptr::null_mut(), core::ptr::null_mut()]);
    let mut store = Option::default();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __take_and_return_array_of_opaques(
                input.clone().into_ffi(&mut store),
                output.as_mut_ptr()
            )
        );
        let output = output.assume_init();
        let output = <[OpaqueStruct; 2]>::try_from_ffi(output, &mut ()).unwrap();
        assert_eq!(input, output);
    }
}

#[test]
fn borrow_vec() {
    let a = vec![1, 2];

    let mut store = Default::default();

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __take_vec_ref(<&Vec<u8>>::into_ffi(&a, &mut store))
        );
    }
}

#[test]
fn return_reference_from_slice() {
    let a = vec![1, 2];

    let mut output = MaybeUninit::new(core::ptr::null());

    unsafe {
        assert_eq!(
            FfiReturn::Ok,
            __reference_from_slice(<&[u8]>::into_ffi(&a, &mut ()), output.as_mut_ptr())
        );

        let output: &u8 = iroha_ffi::FfiOutPtrRead::try_read_out(output.assume_init()).unwrap();
        assert_eq!(output, &a[0]);
    }
}

#[test]
fn borrow_local() {
    let a = (1_u8, 2_u8);

    let b: LocalRef<(u8, u8)> = {
        let mut store = Default::default();
        let mut output = MaybeUninit::new(FfiTuple2(0, 0));

        unsafe {
            assert_eq!(
                FfiReturn::Ok,
                __take_tuple_ref(<&(u8, u8)>::into_ffi(&a, &mut store), output.as_mut_ptr())
            );

            FfiOutPtrRead::try_read_out(output.assume_init()).expect("Valid")
        }
    };

    assert_eq!(*b, (1, 2));
}
