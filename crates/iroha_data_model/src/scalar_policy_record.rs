//! One field kernel for committed policy records whose fields are all scalars.
//!
//! [`scalar_policy_record!`] declares the record, its ordinary and
//! original-pool JSON decoders (every field required, duplicate and unknown
//! keys refused), its field names and a field-level difference. The record is
//! `Copy` and retains no allocation, so the admitted decoder needs the caller's
//! pool only for the scratch of an escaped key.

/// Declare a scalar committed policy record named `$name`.
///
/// `$label` names the policy in decoder messages, for example
/// `"settlement router policy"`, and `$schema` is its Norito schema identity,
/// which a Parameter definition of the record names.
macro_rules! scalar_policy_record {
    (
        $(#[$record_meta:meta])*
        $name:ident, $label:literal, $schema:literal,
        { $( $(#[$field_meta:meta])* $field:ident: $type:ty ),+ $(,)? }
    ) => {
        $(#[$record_meta])*
        #[derive(norito::NoritoSchema)]
        #[norito_schema(name = $schema)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            norito::codec::Encode,
            norito::codec::Decode,
            iroha_schema::IntoSchema,
            $crate::DeriveJsonSerialize,
        )]
        #[norito(deny_unknown_fields)]
        pub struct $name {
            $( $(#[$field_meta])* pub $field: $type, )+
        }

        impl norito::json::JsonDeserialize for $name {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                Self::decode_record(
                    parser,
                    &$crate::pipeline_validity_policy::json_decode::Ordinary,
                )
            }
        }

        impl $name {
            /// Names of the policy fields, in declaration order.
            pub const FIELD_NAMES: &'static [&'static str] = &[ $( stringify!($field) ),+ ];

            /// Names of the fields whose values differ between `self` and `other`.
            #[must_use]
            pub fn differing_fields(&self, other: &Self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $(
                    if self.$field != other.$field {
                        out.push(stringify!($field));
                    }
                )+
                out
            }

            /// Decode this scalar record directly with original-pool escaped-key scratch.
            ///
            /// Ordinary and admitted reads share this one field kernel. The
            /// result is Copy and retains no allocation. Call `validate` before
            /// installation; parsing does not grant policy validity or
            /// committed State authority.
            ///
            /// # Errors
            /// Preserves native JSON observer, finite-pool and physical allocator errors.
            pub fn try_decode_json(
                parser: &mut norito::json::Parser<'_>,
                budget: &iroha_allocation::AllocationBudget,
            ) -> Result<Self, $crate::pipeline_validity_policy::PipelinePolicyJsonAdmissionError>
            {
                Self::decode_record(
                    parser,
                    &$crate::pipeline_validity_policy::json_decode::Admitted { budget },
                )
            }

            fn decode_record<D: $crate::pipeline_validity_policy::json_decode::Destination>(
                parser: &mut norito::json::Parser<'_>,
                destination: &D,
            ) -> Result<Self, D::Error> {
                use $crate::pipeline_validity_policy::json_decode::{invalid, key_text};
                $(let mut $field = None;)+
                let mut object = norito::json::MapVisitor::new(parser)?;
                while let Some(key) = destination.key(&mut object)? {
                    match key_text(&key) {
                        $(stringify!($field) => {
                            if $field.is_some() {
                                return Err(invalid(concat!(
                                    "duplicate ", $label, " field: ", stringify!($field)
                                ))
                                .into());
                            }
                            $field = Some(object.parse_value_with_parser_typed(|parser| {
                                <$type as norito::json::JsonDeserialize>::json_deserialize(parser)
                                    .map_err(D::Error::from)
                            })?);
                        },)+
                        _ => return Err(invalid(concat!("unknown ", $label, " field")).into()),
                    }
                }
                object.finish()?;
                Ok(Self {
                    $($field: $field.ok_or_else(|| {
                        invalid(concat!("missing ", $label, " field: ", stringify!($field)))
                    })?,)+
                })
            }
        }
    };
}
pub(crate) use scalar_policy_record;
