//! A crate containing various derive macros for `iroha_data_model`
// darling-generated code triggers this lint
#![allow(clippy::needless_continue)]
mod emitter_ext;
mod enum_ref;
mod event_set;
mod has_origin;
mod id;
mod model;
mod registrable_builder;
mod utils;

use darling::{FromMeta, ast::NestedMeta};
use emitter_ext::EmitterExt;
use manyhow::{Emitter, Result, emit, manyhow};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

use crate::utils::darling_error;

/// Construct a matching enum with references in place of enum variant fields
///
/// # Example
///
/// ```
/// mod model {
///     use iroha_data_model_derive::EnumRef;
///     use norito::codec::Encode;
///
///     #[derive(EnumRef)]
///     #[enum_ref(derive(Encode))]
///     pub enum InnerEnum {
///         A(u32),
///         B(i32),
///     }
///
///     #[derive(EnumRef)]
///     #[enum_ref(derive(Encode))]
///     pub enum OuterEnum {
///         A(String),
///         #[enum_ref(transparent)]
///         B(InnerEnum),
///     }
/// }
///
/// /* will produce:
/// mod model {
///     #[derive(Encode)]
///     pub(super) enum InnerEnumRef<'a> {
///         A(&'a u32),
///         B(&'a i32),
///     }
///
///     #[derive(Encode)]
///     pub(super) enum OuterEnumRef<'a> {
///         A(&'a String),
///         B(InnerEnumRef<'a>),
///     }
/// }
/// */
/// ```
#[manyhow]
#[proc_macro_derive(EnumRef, attributes(enum_ref))]
pub fn enum_ref(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    enum_ref::impl_enum_ref(&input)
}

/// Macro which controls how to export item's API. The behaviour is controlled with `transparent_api`
/// feature flag. If the flag is active, item's public fields will be exposed as public, however, if
/// it's not active, item will be exposed as opaque, i.e. no fields will be visible. This enables
/// internal libraries of Iroha to see and destructure data model items. On the other hand,
/// client libraries will only see opaque items and can be dynamically linked.
///
/// Additionally, this macro will rewrite private items as public when `transparent_api` is active.
/// If an item should remain private regardless of consumer library, just don't wrap it in this macro.
///
/// Should be used only on public module named `model`.
/// Macro will modify only structs, enums and unions. Other items will be left as is.
///
/// # Example
///
/// ```
/// use iroha_data_model_derive::model;
///
/// #[model]
/// mod model {
///     pub struct DataModel1 {
///         pub item1: u32,
///         item2: u64,
///     }
///
///     pub(crate) struct DataModel2 {
///         pub item1: u32,
///         item2: u64,
///     }
/// }
///
/// /* will produce:
/// mod model {
///     pub struct DataModel1 {
///         #[cfg(feature = "transparent_api")]
///         pub item1: u32,
///         #[cfg(not(feature = "transparent_api"))]
///         pub(crate) item1: u32,
///         item2: u64
///     }
///
///     #[cfg(not(feature = "transparent_api"))]
///     pub struct DataModel2 {
///         pub item1: u32,
///         item2: u64
///     }
///
///     #[cfg(feature = "transparent_api")]
///     struct DataModel2 {
///         pub item1: u32,
///         item2: u64
///     }
/// }
/// */
/// ```
///
/// ## A note on `#[derive(...)]` limitations
///
/// This proc-macro crate parses the `#[derive(...)]` attributes.
/// Due to technical limitations of proc macros, it does not have access to the resolved path of the macro, only to what is written in the derive.
/// As such, it cannot support derives that are used through aliases, such as
///
/// ```ignore
/// use getset::Getters as GettersAlias;
/// #[derive(GettersAlias)]
/// pub struct Hello {
///     // ...
/// }
/// ```
///
/// It assumes that the derive is imported and referred to by its original name.
#[manyhow]
#[proc_macro_attribute]
pub fn model(attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    if !attr.is_empty() {
        emit!(emitter, attr, "This attribute does not take any arguments");
    }

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    let result = model::impl_model(&mut emitter, &input);

    emitter.finish_token_stream_with(result)
}

/// Same as [`model()`] macro, but only processes a single item.
///
/// You should prefer using [`model()`] macro over this one.
#[manyhow]
#[proc_macro]
pub fn model_single(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    emitter.finish_token_stream_with(model::process_item(input))
}

/// Attribute macro to attach a stable wire identifier to an instruction type.
///
/// Usage: `#[instruction(id = "iroha.log")]` applied to a struct/enum.
/// It generates an inherent associated constant `WIRE_ID: &str` on the type.
///
/// This macro does not alter registration automatically; the registry can
/// choose to use the constant, or you can pass the same ID into
/// `InstructionRegistry::register_with_id::<T>(id)`.
#[manyhow]
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    #[derive(FromMeta)]
    struct Args {
        id: String,
    }
    let metas = NestedMeta::parse_meta_list(attr.clone())?;
    let args = Args::from_list(&metas).map_err(darling_error)?;

    let item: Item = syn::parse2(input.clone())?;
    let (ident, generics) = match &item {
        Item::Struct(s) => (s.ident.clone(), s.generics.clone()),
        Item::Enum(e) => (e.ident.clone(), e.generics.clone()),
        _ => return Ok(quote! { #item }),
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let wire_id_lit = syn::LitStr::new(&args.id, proc_macro2::Span::call_site());

    let expanded = quote! {
        #item
        impl #impl_generics #ident #ty_generics #where_clause {
            /// Stable wire identifier for instruction encoding
            pub const WIRE_ID: &'static str = #wire_id_lit;
        }
    };
    Ok(expanded)
}

/// Derive macro for `Identifiable` trait which also automatically implements [`Ord`], [`Eq`],
/// and [`Hash`] for the annotated struct by delegating to it's identifier field. Identifier
/// field for the struct can be selected by annotating the desired field with `#[id]` or
/// `#[id(transparent)]`. The use of `transparent` assumes that the field is also `Identifiable`,
/// and the macro takes the field identifier of the annotated structure. In the absence
/// of any helper attribute, the macro uses the field named `id` if there is such a field.
/// Otherwise, the macro expansion fails.
///
/// The macro should never be used on structs that aren't uniquely identifiable
///
/// # Examples
///
/// The common use-case:
///
/// ```
/// use iroha_data_model::{IdBox, Identifiable};
/// use iroha_data_model_derive::IdEqOrdHash;
///
/// #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// struct Id {
///     name: u32,
/// }
///
/// #[derive(Debug, IdEqOrdHash)]
/// struct Struct {
///     id: Id,
/// }
///
/// # impl From<Id> for IdBox {
/// #     fn from(_source: Id) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// /* which will expand into:
/// impl Identifiable for Struct {
///     type Id = Id;
///
///     #[inline]
///     fn id(&self) -> &Self::Id {
///         &self.id
///     }
/// }
///
/// impl core::cmp::PartialOrd for Struct {
///     #[inline]
///     fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
///         Some(self.cmp(other))
///     }
/// }
///
/// impl core::cmp::Ord for Struct {
///     fn cmp(&self, other: &Self) -> core::cmp::Ordering {
///         self.id().cmp(other.id())
///     }
/// }
///
/// impl core::cmp::PartialEq for Struct {
///     fn eq(&self, other: &Self) -> bool {
///         self.id() == other.id()
///     }
/// }
///
/// impl core::cmp::Eq for Struct {}
///
/// impl core::hash::Hash for Struct {
///     fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
///         self.id().hash(state);
///     }
/// }*/
/// ```
///
/// Manual selection of the identifier field:
///
/// ```
/// use iroha_data_model::{IdBox, Identifiable};
/// use iroha_data_model_derive::IdEqOrdHash;
///
/// #[derive(Debug, IdEqOrdHash)]
/// struct InnerStruct {
///     #[id]
///     field: Id,
/// }
///
/// # impl From<Id> for IdBox {
/// #     fn from(_source: Id) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// #[derive(Debug, IdEqOrdHash)]
/// struct Struct {
///     #[id(transparent)]
///     inner: InnerStruct,
/// }
///
/// # impl From<InnerStruct> for IdBox {
/// #     fn from(_source: InnerStruct) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// # impl From<Struct> for IdBox {
/// #     fn from(_source: Struct) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// struct Id {
///     name: u32,
/// }
/// ```
#[manyhow]
#[proc_macro_derive(IdEqOrdHash, attributes(id, opaque))]
pub fn id_eq_ord_hash(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    let result = id::impl_id_eq_ord_hash(&mut emitter, &input);
    emitter.finish_token_stream_with(result)
}

/// Derive macro for `HasOrigin`.
///
/// Works only with enums containing single unnamed fields.
///
/// # Attributes
///
/// ## Container attributes
///
/// ### `#[has_origin(origin = Type)]`
///
/// Required attribute. Used to determine type of `Origin` in `HasOrigin` trait.
///
/// ## Field attributes
///
/// ### `#[has_origin(ident => expr)]`
///
/// This attribute is used to determine how to extract origin id from enum variant.
/// By default variant is assumed to by origin id.
///
/// # Examples
///
/// ```
/// use iroha_data_model::prelude::{HasOrigin, IdBox, Identifiable};
/// use iroha_data_model_derive::{HasOrigin, IdEqOrdHash};
///
/// #[derive(Debug, Clone, HasOrigin)]
/// #[has_origin(origin = Layer)]
/// pub enum LayerEvent {
///     #[has_origin(sub_layer_event => &sub_layer_event.origin().parent)]
///     SubLayer(SubLayerEvent),
///     Created(LayerId),
/// }
///
/// #[derive(Debug, Clone, HasOrigin)]
/// #[has_origin(origin = SubLayer)]
/// pub enum SubLayerEvent {
///     Created(SubLayerId),
/// }
///
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// pub struct LayerId {
///     name: u32,
/// }
///
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// pub struct SubLayerId {
///     name: u32,
///     parent: LayerId,
/// }
///
/// #[derive(Debug, Clone, IdEqOrdHash)]
/// pub struct Layer {
///     id: LayerId,
/// }
///
/// #[derive(Debug, Clone, IdEqOrdHash)]
/// pub struct SubLayer {
///     id: SubLayerId,
/// }
///
/// # impl From<LayerId> for IdBox {
/// #     fn from(_source: LayerId) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// # impl From<SubLayerId> for IdBox {
/// #     fn from(_source: SubLayerId) -> Self {
/// #         unimplemented!("Only present to make the example work")
/// #     }
/// # }
///
/// let layer_id = LayerId { name: 42 };
/// let sub_layer_id = SubLayerId {
///     name: 24,
///     parent: layer_id.clone(),
/// };
/// let layer_created_event = LayerEvent::Created(layer_id.clone());
/// let sub_layer_created_event = SubLayerEvent::Created(sub_layer_id.clone());
/// let layer_sub_layer_event = LayerEvent::SubLayer(sub_layer_created_event.clone());
///
/// assert_eq!(&layer_id, layer_created_event.origin());
/// assert_eq!(&layer_id, layer_sub_layer_event.origin());
/// assert_eq!(&sub_layer_id, sub_layer_created_event.origin());
/// ```
#[manyhow]
#[proc_macro_derive(HasOrigin, attributes(has_origin))]
pub fn has_origin_derive(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    let result = has_origin::impl_has_origin(&mut emitter, &input);

    emitter.finish_token_stream_with(result)
}

/// Create an event set structure from an event enum.
///
/// Event set is a set of multiple event types, allowing to efficiently test whether an event is in a set.
///
/// For this event enum:
///
/// ```rust
/// # use iroha_data_model_derive::EventSet;
/// #[derive(EventSet)]
/// pub enum TestEvent {
///     Event1,
///     Event2,
///     NestedEvent(AnotherEvent),
/// }
/// pub struct AnotherEvent;
/// ```
///
/// The macro will generate a `TestEventSet` struct.
///
/// It will have associated constants that correspond to a singleton set for each event that can be accessed like `TestEventSet::Event1`.
///
/// The sets can be combined either with a `|` operator or with the `or` method to match multiple events at once: `TestEventSet::Event1 | TestEventSet::Event2`.
///
/// Variants that:
/// 1. have a single unnamed field
/// 2. have the type name end with `Event`
///
/// are treated as nested events and their set constant has `Any` prepended to the name. For example, `TestEventSet::AnyNestedEvent`.
///
/// The set has the following methods:
/// ```ignore
/// impl TestEventSet {
///     /// Returns a set that doesn't contain any events
///     const fn empty() -> TestEventSet;
///     /// Returns a set that contains all events
///     const fn all() -> TestEventSet;
///     /// Computes union of two sets, const form of the `|` operator
///     const fn or(self, other: TestEventSet) -> TestEventSet;
///     /// Checks if the set contains a specific event
///     const fn matches(&self, event: &TestEvent) -> bool;
/// }
/// ```
///
/// Implemented traits:
/// ```ignore
/// #[derive(
///     Copy,
///     Clone,
///     PartialEq,
///     Eq,
///     PartialOrd,
///     Ord,
///     Hash,
///     norito::codec::Decode,
///     norito::codec::Encode,
///     iroha_schema::IntoSchema,
/// )]
///
/// /// Prints the list of events in the set
/// ///
/// /// For example: `TestEventSet[Event1, AnyNestedEvent]`
/// impl core::fmt::Debug;
///
/// /// Union of two sets
/// impl core::ops::BitOr;
/// ```
#[manyhow]
#[proc_macro_derive(EventSet)]
pub fn event_set_derive(input: TokenStream) -> TokenStream {
    let mut emitter = Emitter::new();

    let Some(input) = emitter.handle(syn::parse2(input)) else {
        return emitter.finish_token_stream();
    };

    let result = event_set::impl_event_set_derive(&mut emitter, &input);

    emitter.finish_token_stream_with(result)
}

/// Derive macro generating registration builders for data model structs.
#[manyhow]
#[proc_macro_derive(RegistrableBuilder, attributes(registrable_builder))]
pub fn registrable_builder(input: TokenStream) -> Result<TokenStream> {
    let input = syn::parse2(input)?;
    let mut emitter = Emitter::new();
    let result = registrable_builder::impl_registrable_builder(&mut emitter, &input);
    Ok(emitter.finish_token_stream_with(result))
}
