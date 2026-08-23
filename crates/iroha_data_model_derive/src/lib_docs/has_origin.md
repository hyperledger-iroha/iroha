Derive macro for `HasOrigin`.

Works only with enums containing single unnamed fields.

# Attributes

## Container attributes

### `#[has_origin(origin = Type)]`

Required attribute. Used to determine type of `Origin` in `HasOrigin` trait.

## Field attributes

### `#[has_origin(ident => expr)]`

This attribute is used to determine how to extract origin id from enum variant.
By default variant is assumed to by origin id.

# Examples

```
use iroha_data_model::{
    name::Name,
    parameter::CustomParameterId,
    prelude::{HasOrigin, IdBox, Identifiable},
};
use iroha_data_model_derive::{HasOrigin, IdEqOrdHash};
use std::str::FromStr;

#[derive(Debug, Clone, HasOrigin)]
#[has_origin(origin = Layer)]
pub enum LayerEvent {
    #[has_origin(sub_layer_event => &sub_layer_event.origin().parent)]
    SubLayer(SubLayerEvent),
    Created(LayerId),
}

#[derive(Debug, Clone, HasOrigin)]
#[has_origin(origin = SubLayer)]
pub enum SubLayerEvent {
    Created(SubLayerId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId {
    name: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubLayerId {
    name: u32,
    parent: LayerId,
}

#[derive(Debug, Clone, IdEqOrdHash)]
pub struct Layer {
    id: LayerId,
}

#[derive(Debug, Clone, IdEqOrdHash)]
pub struct SubLayer {
    id: SubLayerId,
}

# impl From<LayerId> for IdBox {
#     fn from(_source: LayerId) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("layer_example").expect("valid parameter id"),
#         ))
#     }
# }

# impl From<SubLayerId> for IdBox {
#     fn from(_source: SubLayerId) -> Self {
#         IdBox::CustomParameterId(CustomParameterId::new(
#             Name::from_str("sub_layer_example").expect("valid parameter id"),
#         ))
#     }
# }

let layer_id = LayerId { name: 42 };
let sub_layer_id = SubLayerId {
    name: 24,
    parent: layer_id.clone(),
};
let layer_created_event = LayerEvent::Created(layer_id.clone());
let sub_layer_created_event = SubLayerEvent::Created(sub_layer_id.clone());
let layer_sub_layer_event = LayerEvent::SubLayer(sub_layer_created_event.clone());

assert_eq!(&layer_id, layer_created_event.origin());
assert_eq!(&layer_id, layer_sub_layer_event.origin());
assert_eq!(&sub_layer_id, sub_layer_created_event.origin());
```
