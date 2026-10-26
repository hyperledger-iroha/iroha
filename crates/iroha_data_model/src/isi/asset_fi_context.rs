//! Owner-scoped current FI and wallet/owner-scoped historical FI assertions.
use super::*;
use crate::{asset::fi_context::AssetFiContextTargetV1, nexus::AxtAssetIncarnationV1};

isi! {
    /// Assert the selected current native FI context without changing money or approval.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::asset_fi_context::AssertAssetFiContextV1")]
    pub struct AssertAssetFiContextV1 {
        /// Complete descriptive operation retained by the consumer.
        pub target: AssetFiContextTargetV1,
        /// Exact expected current asset incarnation.
        pub expected_incarnation: AxtAssetIncarnationV1,
        /// Exact expected UID; an existing wallet binding must agree.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub expected_uid: [u8;32],
        /// Public canonical hash of the independently derived typed snapshot.
        pub expected_context_hash: iroha_crypto::Hash,
        /// Raw SHA-256 of the consumer's exact retained request bytes.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub request_sha256: [u8;32],
        /// Nonzero consumer challenge; freshness is checked by the proof consumer.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub challenge: [u8;32],
        /// Retained checkpoint height strictly before assertion execution.
        pub minimum_height: u64,
        /// Exact raw native block hash at that retained height.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub minimum_block_hash: [u8;32],
    }
}
isi! {
    /// Assert one full immutable receipt without substituting current FI state.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::asset_fi_context::AssertAssetFiTransferReceiptV1")]
    pub struct AssertAssetFiTransferReceiptV1 {
        /// Exact Global definition retained by the occurrence.
        pub asset_definition_id: AssetDefinitionId,
        /// Exact historical incarnation captured in the receipt.
        pub expected_incarnation: AxtAssetIncarnationV1,
        /// Exact source wallet retained in the occurrence.
        pub account_id: AccountId,
        /// Original native execution call identity.
        pub execution_call_hash: iroha_crypto::Hash,
        /// Actual transfer-transcript occurrence ordinal.
        pub transcript_index: u64,
        /// Public canonical hash of the entire original receipt.
        pub expected_receipt_hash: iroha_crypto::Hash,
        /// Raw SHA-256 of exact consumer-retained request bytes.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub request_sha256: [u8;32],
        /// Nonzero consumer challenge.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub challenge: [u8;32],
        /// Exact retained floor strictly before assertion execution.
        pub minimum_height: u64,
        /// Exact raw native hash of that floor block.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        pub minimum_block_hash: [u8;32],
    }
}
// The field list is forwarded as raw tokens: the shared slice decoder must see the `[u8; N]` syntax to
// read the encoder's direct raw byte-array members; a `ty` fragment would make them look canonical.
macro_rules! assertion_instruction {
    ($ty:ident, $wire:literal, {$($fields:tt)+}) => {
        impl $ty {
            /// Canonical first-release instruction wire identity.
            pub const WIRE_ID: &'static str = $wire;
        }
        impl crate::seal::Instruction for $ty {}
        crate::isi::impl_aos_decode_from_slice!($ty { $($fields)+ });
    };
}
assertion_instruction!(AssertAssetFiContextV1, "iroha.asset.fi.context.assert.v1", {
    target: AssetFiContextTargetV1, expected_incarnation: AxtAssetIncarnationV1,
    expected_uid: [u8;32], expected_context_hash: iroha_crypto::Hash,
    request_sha256: [u8;32], challenge: [u8;32], minimum_height: u64, minimum_block_hash: [u8;32]
});
assertion_instruction!(AssertAssetFiTransferReceiptV1, "iroha.asset.fi.transfer_receipt.assert.v1", {
    asset_definition_id: AssetDefinitionId, expected_incarnation: AxtAssetIncarnationV1,
    account_id: AccountId, execution_call_hash: iroha_crypto::Hash, transcript_index: u64,
    expected_receipt_hash: iroha_crypto::Hash, request_sha256: [u8;32], challenge: [u8;32],
    minimum_height: u64, minimum_block_hash: [u8;32]
});
#[cfg(test)]
pub(crate) mod test_values {
    use super::*;
    use crate::{
        asset::fi_link::AssetFiLinkV1,
        isi::asset_fi::{SetAssetFiLinkV1, TransferToLinkedFiV1},
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    pub fn current(transfer: bool) -> AssertAssetFiContextV1 {
        let account = |seed| {
            AccountId::new(
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            )
        };
        let definition = AssetDefinitionId::from_uuid_bytes([
            0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0x48, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8, 0xa8,
            0xa8, 0xa8,
        ])
        .unwrap();
        let link = AssetFiLinkV1 {
            asset_definition_id: definition.clone(),
            account_id: account(91),
            fi_account_id: account(92),
            reference_digest: [37; 32],
            fi_revision: 2,
            attestation_revision: 3,
            revision: 4,
            enabled: true,
            controller: None,
            waterfall: false,
            reverse_waterfall: false,
        };
        let target = if transfer {
            AssetFiContextTargetV1::Transfer {
                instruction: TransferToLinkedFiV1 {
                    asset_definition_id: definition,
                    account_id: link.account_id.clone(),
                    amount: 17_u32.into(),
                    expected_link_revision: 4,
                    expected_policy_revision: 5,
                },
                fi_account_id: link.fi_account_id.clone(),
                reference_digest: link.reference_digest,
                expected_fi_revision: link.fi_revision,
                expected_attestation_revision: link.attestation_revision,
            }
        } else {
            AssetFiContextTargetV1::LinkApproval(SetAssetFiLinkV1 {
                link,
                expected_policy_revision: 5,
            })
        };
        AssertAssetFiContextV1 {
            target,
            expected_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new("fi-context-fixture-incarnation").as_ref(),
            )
            .unwrap(),
            expected_uid: [7; 32],
            expected_context_hash: Hash::new("fi-context-fixture-context"),
            request_sha256: [0x80; 32],
            challenge: [0x81; 32],
            minimum_height: 17,
            minimum_block_hash: *Hash::new("fi-context-fixture-floor").as_ref(),
        }
    }
    pub fn historical() -> AssertAssetFiTransferReceiptV1 {
        let current = current(false);
        let AssetFiContextTargetV1::LinkApproval(value) = current.target else {
            unreachable!()
        };
        AssertAssetFiTransferReceiptV1 {
            asset_definition_id: value.link.asset_definition_id,
            account_id: value.link.account_id,
            expected_incarnation: current.expected_incarnation,
            execution_call_hash: Hash::new("fi-context-original-call"),
            transcript_index: u64::MAX,
            expected_receipt_hash: Hash::new("fi-context-original-receipt"),
            request_sha256: [0x82; 32],
            challenge: [0x83; 32],
            minimum_height: 18,
            minimum_block_hash: current.minimum_block_hash,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::fi_context::{
        AssetFiContextSnapshotV1, asset_fi_context_snapshot_hash_v1,
        asset_fi_context_target_hash_v1,
    };
    use crate::isi::test_support::{assert_registry_decodes, assert_slice_roundtrip};
    use norito::core::DecodeFromSlice;

    #[test]
    fn fi_assertion_typed_targets_registry_and_raw_request_digest_round_trip() {
        let registry = crate::isi::registry::default();
        for transfer in [false, true] {
            let value = test_values::current(transfer);
            assert_slice_roundtrip(value.clone());
            assert_registry_decodes(&registry, AssertAssetFiContextV1::WIRE_ID, value.clone());
            let encoded = norito::encode_canonical(&value).unwrap();
            assert_eq!(
                norito::decode_canonical::<AssertAssetFiContextV1>(&encoded).unwrap(),
                value
            );
            let json = norito::json::to_json(&value).unwrap();
            assert_eq!(
                norito::json::from_str::<AssertAssetFiContextV1>(&json).unwrap(),
                value
            );
            assert_eq!(value.request_sha256, [0x80; 32]);
            let mut trailing = value.encode();
            trailing.push(0);
            assert!(AssertAssetFiContextV1::decode_from_slice(&trailing).is_err());
            let mut missing = value.encode();
            missing.pop();
            assert!(AssertAssetFiContextV1::decode_from_slice(&missing).is_err());
        }
        let historical = test_values::historical();
        assert_slice_roundtrip(historical.clone());
        assert_registry_decodes(
            &registry,
            AssertAssetFiTransferReceiptV1::WIRE_ID,
            historical.clone(),
        );
        let encoded = norito::encode_canonical(&historical).unwrap();
        assert_eq!(
            norito::decode_canonical::<AssertAssetFiTransferReceiptV1>(&encoded).unwrap(),
            historical
        );
        let json = norito::json::to_json(&historical).unwrap();
        assert_eq!(
            norito::json::from_str::<AssertAssetFiTransferReceiptV1>(&json).unwrap(),
            historical
        );
        let mut trailing = historical.encode();
        trailing.push(0);
        assert!(AssertAssetFiTransferReceiptV1::decode_from_slice(&trailing).is_err());
    }
    #[test]
    fn fi_assertions_preserve_raw_array_fields_under_both_length_layouts() {
        fn check<T>(value: T, raw_fields: &[(usize, [u8; 32])])
        where
            T: norito::core::SerializePayload + PartialEq + core::fmt::Debug,
            for<'a> T: DecodeFromSlice<'a>,
        {
            for flags in [0, norito::core::header_flags::COMPACT_LEN] {
                let _flags = norito::core::DecodeFlagsGuard::enter(flags);
                let mut bytes = Vec::new();
                norito::core::serialize_to_buffer(&value, &mut bytes).unwrap();
                let (decoded, used) = T::decode_from_slice(&bytes).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(used, bytes.len());
                let mut offset = 0;
                let mut field_index = 0;
                let mut checked = 0;
                while offset < bytes.len() {
                    let start = offset;
                    let field = crate::isi::read_aos_field(&bytes, &mut offset, flags).unwrap();
                    if let Some((_, expected)) =
                        raw_fields.iter().find(|(index, _)| *index == field_index)
                    {
                        assert_eq!(field, expected);
                        checked += 1;
                        for width in [31_u64, 33] {
                            let mut malformed = bytes[..start].to_vec();
                            norito::core::write_len_to_vec_with_flags(&mut malformed, width, flags);
                            malformed.extend_from_slice(&field[..field.len().min(width as usize)]);
                            if width == 33 {
                                malformed.push(0);
                            }
                            malformed.extend_from_slice(&bytes[offset..]);
                            assert!(matches!(
                                T::decode_from_slice(&malformed),
                                Err(norito::core::Error::LengthMismatch)
                            ));
                        }
                    }
                    field_index += 1;
                }
                assert_eq!(checked, raw_fields.len());
                let mut trailing = bytes.clone();
                trailing.push(0);
                assert!(T::decode_from_slice(&trailing).is_err());
                assert!(T::decode_from_slice(&bytes[..bytes.len() - 1]).is_err());
            }
        }
        for transfer in [false, true] {
            let value = test_values::current(transfer);
            let raw_fields = [
                (2, value.expected_uid),
                (4, value.request_sha256),
                (5, value.challenge),
                (7, value.minimum_block_hash),
            ];
            check(value, &raw_fields);
        }
        let value = test_values::historical();
        let raw_fields = [
            (6, value.request_sha256),
            (7, value.challenge),
            (9, value.minimum_block_hash),
        ];
        check(value, &raw_fields);
    }

    #[test]
    fn fi_public_hashes_commit_typed_selectors_revisions_and_explicit_absence() {
        let value = test_values::current(true);
        let target_hash = asset_fi_context_target_hash_v1(&value.target).unwrap();
        assert_eq!(
            target_hash,
            iroha_crypto::Hash::new(norito::encode_canonical(&value.target).unwrap())
        );
        for selector in 0..5 {
            let mut target = value.target.clone();
            let AssetFiContextTargetV1::Transfer {
                instruction,
                fi_account_id,
                reference_digest,
                expected_fi_revision,
                expected_attestation_revision,
            } = &mut target
            else {
                unreachable!()
            };
            match selector {
                0 => instruction.amount = 18_u32.into(),
                1 => *fi_account_id = instruction.account_id.clone(),
                2 => *reference_digest = [38; 32],
                3 => *expected_fi_revision += 1,
                _ => *expected_attestation_revision += 1,
            }
            assert_ne!(
                asset_fi_context_target_hash_v1(&target).unwrap(),
                target_hash
            );
        }
        let AssetFiContextTargetV1::Transfer {
            instruction,
            fi_account_id,
            reference_digest,
            ..
        } = value.target
        else {
            unreachable!()
        };
        let snapshot = AssetFiContextSnapshotV1 {
            asset_definition_id: instruction.asset_definition_id,
            asset_incarnation: value.expected_incarnation,
            account_id: instruction.account_id,
            uid: value.expected_uid,
            fi_account_id,
            reference_digest,
            binding: None,
            policy: None,
            fi: None,
            attestation: None,
            link: None,
            controller: None,
        };
        let hash = asset_fi_context_snapshot_hash_v1(&snapshot).unwrap();
        assert_eq!(
            hash,
            iroha_crypto::Hash::new(norito::encode_canonical(&snapshot).unwrap())
        );
        let encoded = norito::encode_canonical(&snapshot).unwrap();
        assert_eq!(
            norito::decode_canonical::<AssetFiContextSnapshotV1>(&encoded).unwrap(),
            snapshot
        );
        let mut changed = snapshot.clone();
        changed.uid = [8; 32];
        assert_ne!(asset_fi_context_snapshot_hash_v1(&changed).unwrap(), hash);
        let json = norito::json::to_json(&snapshot).unwrap();
        let missing = json
            .replace("\"controller\":null,", "")
            .replace(",\"controller\":null", "");
        assert_ne!(json, missing);
        assert!(norito::json::from_str::<AssetFiContextSnapshotV1>(&missing).is_err());
    }

    #[test]
    fn fi_assertion_raw_byte_fields_reject_wrong_widths_and_nested_array_encodings() {
        use norito::core::{
            DecodeFlagsGuard, Error, SerializePayload, header_flags,
            read_len_from_slice_with_flags, serialize_to_buffer, write_len_to_vec_with_flags,
        };

        fn check<T>(value: T, raw_fields: &[usize], field_count: usize, flags: u8)
        where
            T: SerializePayload + PartialEq + core::fmt::Debug,
            for<'a> T: DecodeFromSlice<'a>,
        {
            let _flags = DecodeFlagsGuard::enter(flags);
            let mut wire = Vec::new();
            serialize_to_buffer(&value, &mut wire).unwrap();
            let (decoded, used) = T::decode_from_slice(&wire).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(used, wire.len());

            // Walk actual field prefixes rather than locating repeated digest
            // bytes. Keep all other fields byte-identical in every mutation.
            let mut offset = 0usize;
            let mut spans = Vec::new();
            while offset < wire.len() {
                let prefix = offset;
                let (length, header) =
                    read_len_from_slice_with_flags(&wire[offset..], flags).unwrap();
                let body = offset.checked_add(header).unwrap();
                let end = body.checked_add(length).unwrap();
                assert!(end <= wire.len());
                spans.push((prefix, body, end));
                offset = end;
            }
            assert_eq!(spans.len(), field_count);
            assert_eq!(offset, wire.len());

            for &field in raw_fields {
                let (prefix, body, end) = spans[field];
                let original = &wire[body..end];
                assert_eq!(original.len(), 32);
                let short = original[..31].to_vec();
                let mut long = original.to_vec();
                long.push(0xa5);
                let mut element_framed = Vec::new();
                let array: [u8; 32] = original.try_into().unwrap();
                serialize_to_buffer(&array, &mut element_framed).unwrap();
                assert_ne!(element_framed.as_slice(), original);

                for replacement in [short, long, element_framed] {
                    let mut malformed = wire[..prefix].to_vec();
                    write_len_to_vec_with_flags(
                        &mut malformed,
                        u64::try_from(replacement.len()).unwrap(),
                        flags,
                    );
                    malformed.extend_from_slice(&replacement);
                    malformed.extend_from_slice(&wire[end..]);
                    assert!(
                        matches!(T::decode_from_slice(&malformed), Err(Error::LengthMismatch)),
                        "raw field {field}, flags {flags:#x}, replacement width {}",
                        replacement.len(),
                    );
                }
            }
        }

        for flags in [0, header_flags::COMPACT_LEN] {
            for transfer in [false, true] {
                // expected_uid, request_sha256, challenge, minimum_block_hash.
                check(test_values::current(transfer), &[2, 4, 5, 7], 8, flags);
            }
            // request_sha256, challenge, minimum_block_hash.
            check(test_values::historical(), &[6, 7, 9], 10, flags);
        }
    }
}
