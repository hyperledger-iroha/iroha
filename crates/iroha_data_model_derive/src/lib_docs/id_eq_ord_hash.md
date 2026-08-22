Derive macro for `Identifiable` trait which also automatically implements [`Ord`], [`Eq`], and
[`Hash`] for the annotated struct by delegating to it's identifier field. Identifier field for
the struct can be selected by annotating the desired field with `#[id]` or `#[id(transparent)]`.
The use of `transparent` assumes that the field is also `Identifiable`, and the macro takes the
field identifier of the annotated structure. In the absence of any helper attribute, the macro
uses the field named `id` if there is such a field. Otherwise, the macro expansion fails.

The macro should never be used on structs that aren't uniquely identifiable

# Examples

The common use-case:

```
use iroha_data_model::{IdBox, Identifiable, name::Name, parameter::CustomParameterId};
use iroha_data_model_derive::IdEqOrdHash;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Id {
    name: u32,
}

#[derive(Debug, IdEqOrdHash)]
struct Struct {
    id: Id,
}

# impl From<Id> for IdBox {
#     fn from(_source: Id) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("id_eq_ord_hash_example").expect("valid parameter id"),
#         ))
#     }
# }

/* which will expand into:
impl Identifiable for Struct {
    type Id = Id;

    #[inline]
    fn id(&self) -> &Self::Id {
        &self.id
    }
}

impl core::cmp::PartialOrd for Struct {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::cmp::Ord for Struct {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl core::cmp::PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl core::cmp::Eq for Struct {}

impl core::hash::Hash for Struct {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}*/
```

Manual selection of the identifier field:

```
use iroha_data_model::{IdBox, Identifiable, name::Name, parameter::CustomParameterId};
use iroha_data_model_derive::IdEqOrdHash;
use std::str::FromStr;

#[derive(Debug, IdEqOrdHash)]
struct InnerStruct {
    #[id]
    field: Id,
}

# impl From<Id> for IdBox {
#     fn from(_source: Id) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("inner_id_eq_ord_hash_example").expect("valid parameter id"),
#         ))
#     }
# }

#[derive(Debug, IdEqOrdHash)]
struct Struct {
    #[id(transparent)]
    inner: InnerStruct,
}

# impl From<InnerStruct> for IdBox {
#     fn from(_source: InnerStruct) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("inner_struct_example").expect("valid parameter id"),
#         ))
#     }
# }

# impl From<Struct> for IdBox {
#     fn from(_source: Struct) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("struct_example").expect("valid parameter id"),
#         ))
#     }
# }

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Id {
    name: u32,
}
```
