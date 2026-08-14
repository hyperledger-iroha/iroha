//! Self-delimiting type-classification tests.
use super::*;
#[test]
fn allowlisted_types_are_self_delimiting() {
    let ok_types: Vec<syn::Type> = vec![
        syn::parse_quote!(String),
        syn::parse_quote!(Cow<'static, str>),
        syn::parse_quote!(PhantomData<u8>),
        syn::parse_quote!(Vec<u8>),
        syn::parse_quote!(VecDeque<u8>),
        syn::parse_quote!(LinkedList<u8>),
        syn::parse_quote!(BinaryHeap<u8>),
        syn::parse_quote!(HashMap<String, u8>),
        syn::parse_quote!(BTreeMap<String, u8>),
        syn::parse_quote!(HashSet<u8>),
        syn::parse_quote!(BTreeSet<u8>),
        syn::parse_quote!(Option<u32>),
        syn::parse_quote!(Result<u8, u8>),
    ];
    for ty in ok_types {
        assert!(is_self_delimiting(&ty), "expected self-delimiting: {ty:?}");
    }
}
#[test]
fn non_allowlisted_types_are_not_self_delimiting() {
    let bad_types: Vec<syn::Type> = vec![
        syn::parse_quote!(u32),
        syn::parse_quote!(Foo),
        syn::parse_quote!(ConstVec<u8>),
        syn::parse_quote!(Name),
        syn::parse_quote!(Metadata),
        syn::parse_quote!(ProofAttachment),
        syn::parse_quote!(VerifyingKeyId),
        syn::parse_quote!(ConstString),
        syn::parse_quote!(ViewChangeProofPayload),
    ];
    for ty in bad_types {
        assert!(
            !is_self_delimiting(&ty),
            "unexpected self-delimiting: {ty:?}"
        );
    }
}
