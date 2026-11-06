// Explicit Ironwood recipient request. Public recipient/memo intent is independently bound before custody is consumed.
/// Reject impossible public output shapes before a single-use credential is consumed.
pub(crate) fn validate_ironwood_output_shape_v1(
    bytes: &[u8],
) -> Result<(), PrivacyWalletBundleErrorV1> {
    let action = json_object(bytes, MAX_PUBLIC_ACTION_BYTES, "ironwood-public-action")?;
    let count = |field: &str| {
        action
            .get(field)
            .and_then(norito::json::Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| PrivacyWalletBundleErrorV1::at("ironwood-output-shape"))
    };
    if count("outputs")?
        .checked_add(count("changes")?)
        .is_none_or(|n| n > iroha_core_privacy::privacy_engines::orchard::IRONWOOD_MAX_ACTIONS_V1)
    {
        return Err(PrivacyWalletBundleErrorV1::at("ironwood-output-shape"));
    }
    Ok(())
}

struct IronwoodSecretValue(norito::json::Value);
impl Drop for IronwoodSecretValue {
    fn drop(&mut self) {
        zeroize_json_value(core::mem::replace(&mut self.0, norito::json::Value::Null));
    }
}
impl IronwoodSecretValue {
    fn object(
        mut self,
        fields: &[&str],
        stage: &'static str,
    ) -> Result<SecretJsonObject, PrivacyWalletBundleErrorV1> {
        if !self.0.is_object() {
            return Err(PrivacyWalletBundleErrorV1::at(stage));
        }
        let norito::json::Value::Object(object) =
            core::mem::replace(&mut self.0, norito::json::Value::Null)
        else {
            unreachable!()
        };
        let object = SecretJsonObject(object);
        exact_fields(&object.0, fields, stage)?;
        Ok(object)
    }
}
fn ironwood_secret_object(bytes: &[u8]) -> Result<SecretJsonObject, PrivacyWalletBundleErrorV1> {
    let invalid = || PrivacyWalletBundleErrorV1::at("ironwood-witness");
    if bytes.is_empty()
        || bytes.len() > PRIVACY_NATIVE_ACTION_MAX_SECRET_BUNDLE_BYTES_V1
        || bytes.contains(&0)
    {
        return Err(invalid());
    }
    let value = IronwoodSecretValue(
        norito::json::parse_value(core::str::from_utf8(bytes).map_err(|_| invalid())?)
            .map_err(|_| invalid())?,
    );
    let canonical = Zeroizing::new(norito::json::to_json(&value.0).map_err(|_| invalid())?);
    if canonical.as_bytes() != bytes {
        return Err(invalid());
    }
    value.object(&["changes", "spends"], "ironwood-witness")
}

fn take_ironwood_secret_array(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<Vec<IronwoodSecretValue>, PrivacyWalletBundleErrorV1> {
    let mut value = IronwoodSecretValue(take_value(object, field, stage)?);
    let array = value
        .0
        .as_array_mut()
        .filter(|a| a.len() <= 2)
        .ok_or_else(|| PrivacyWalletBundleErrorV1::at(stage))?;
    Ok(array.drain(..).map(IronwoodSecretValue).collect())
}

fn take_ironwood_secret_hex<const N: usize>(
    object: &mut norito::json::Map,
    field: &str,
    stage: &'static str,
) -> Result<Zeroizing<[u8; N]>, PrivacyWalletBundleErrorV1> {
    let value = take_value(object, field, stage)?;
    let norito::json::Value::String(value) = value else {
        zeroize_json_value(value);
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    };
    let value = Zeroizing::new(value);
    let mut decoded = Zeroizing::new([0; N]);
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || hex::decode_to_slice(value.as_bytes(), decoded.as_mut()).is_err()
        || decoded.iter().all(|b| *b == 0)
    {
        return Err(PrivacyWalletBundleErrorV1::at(stage));
    }
    Ok(decoded)
}
fn decode_ironwood_request_v1(
    public_action: &[u8],
    protocol_witness: &[u8],
) -> Result<PrivacyNativeActionRequestV1, PrivacyWalletBundleErrorV1> {
    validate_ironwood_output_shape_v1(public_action)?;
    let mut public = json_object(
        public_action,
        MAX_PUBLIC_ACTION_BYTES,
        "ironwood-public-action",
    )?;
    exact_fields(
        &public,
        &[
            "anchor_epoch",
            "anchor_hex",
            "asset_definition_id",
            "expiry_height",
            "minimum_action_count",
            "pool_id_hex",
            "public_balance_scope",
            "public_debit_decimal",
            "outputs",
            "changes",
        ],
        "ironwood-public-action",
    )?;
    let asset_definition_id = asset_definition_id(
        take_text(&mut public, "asset_definition_id", 512, "ironwood-asset")?,
        "ironwood-asset",
    )?;
    let public_balance_scope = parse_canonical_public_balance_scope_v1(&take_text(
        &mut public,
        "public_balance_scope",
        30,
        "ironwood-public-balance-scope",
    )?)
    .ok_or_else(|| PrivacyWalletBundleErrorV1::at("ironwood-public-balance-scope"))?;
    let pool_id = PrivacyPoolIdV1::new(take_hex(
        &mut public,
        "pool_id_hex",
        "ironwood-pool",
        false,
    )?);
    let anchor_bytes = take_hex(&mut public, "anchor_hex", "ironwood-anchor", false)?;
    let anchor = PrivacyRootV1::new(anchor_bytes);
    let anchor_epoch = take_u64(&mut public, "anchor_epoch", "ironwood-anchor-epoch")?;
    let expiry_height = take_u64(&mut public, "expiry_height", "ironwood-expiry-height")?;
    let minimum_action_count = take_u8(
        &mut public,
        "minimum_action_count",
        "ironwood-minimum-action-count",
    )?;
    if !(1..=2).contains(&minimum_action_count) {
        return Err(PrivacyWalletBundleErrorV1::at(
            "ironwood-minimum-action-count",
        ));
    }
    let public_debit = u64::try_from(take_decimal_u128(
        &mut public,
        "public_debit_decimal",
        "ironwood-public-debit",
        true,
    )?)
    .map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-public-debit"))?;
    let mut outputs = take_array(&mut public, "outputs", 0, 2, "ironwood-outputs")?.into_iter().map(|value| {
        let mut output = value_object(value, &["recipient_hex", "value_decimal", "memo_hex"], "ironwood-output")?;
        let recipient = take_hex(&mut output, "recipient_hex", "ironwood-recipient", false)?;
        let value = u64::try_from(take_decimal_u128(&mut output, "value_decimal", "ironwood-output-value", false)?).map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-output-value"))?;
        let memo = take_hex(&mut output, "memo_hex", "ironwood-memo", true)?;
        iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::output_from_wallet_parts_v1(recipient, value, &memo).map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-output"))
    }).collect::<Result<Vec<_>, _>>()?;
    let mut secret = ironwood_secret_object(protocol_witness)?;
    exact_fields(&secret.0, &["changes", "spends"], "ironwood-witness")?;
    let spends = take_ironwood_secret_array(&mut secret.0, "spends", "ironwood-spends")?
        .into_iter()
        .map(|value| {
            let mut spend = value.object(
                &[
                    "authentication_path_hex",
                    "leaf_position",
                    "random_seed_hex",
                    "recipient_hex",
                    "rho_hex",
                    "spending_key_hex",
                    "value_decimal",
                ],
                "ironwood-spend",
            )?;
            let spending_key = take_ironwood_secret_hex(
                &mut spend.0,
                "spending_key_hex",
                "ironwood-spending-key",
            )?;
            let recipient = take_hex(&mut spend.0, "recipient_hex", "ironwood-recipient", false)?;
            let value = u64::try_from(take_decimal_u128(
                &mut spend.0,
                "value_decimal",
                "ironwood-value",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-value"))?;
            let rho = take_hex(&mut spend.0, "rho_hex", "ironwood-rho", false)?;
            let random_seed =
                take_hex(&mut spend.0, "random_seed_hex", "ironwood-random-seed", false)?;
            let leaf_position = take_u32(&mut spend.0, "leaf_position", "ironwood-leaf-position")?;
            let path = decode_path_v1(
                take_array(
                    &mut spend.0,
                    "authentication_path_hex",
                    32,
                    32,
                    "ironwood-authentication-path",
                )?,
                "ironwood-authentication-path",
            )?;
            iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::spend_from_wallet_parts_v1(
                spending_key,
                recipient,
                value,
                rho,
                random_seed,
                (leaf_position, &path),
                anchor_bytes,
            )
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-spend"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let approved_changes = take_array(&mut public, "changes", 0, 2, "ironwood-approved-changes")?;
    let witness_changes = take_ironwood_secret_array(&mut secret.0, "changes", "ironwood-changes")?;
    if approved_changes.len() != witness_changes.len() {
        return Err(PrivacyWalletBundleErrorV1::at("ironwood-change-approval"));
    }
    let changes = witness_changes.into_iter().zip(approved_changes)
        .map(|(value, approved)| {
            let mut change = value.object(
                &[
                    "diversifier_index",
                    "memo_hex",
                    "scope",
                    "spending_key_hex",
                    "value_decimal",
                ],
                "ironwood-change",
            )?;
            let spending_key = take_ironwood_secret_hex(
                &mut change.0,
                "spending_key_hex",
                "ironwood-change-spending-key",
            )?;
            let internal_scope =
                match take_text(&mut change.0, "scope", 16, "ironwood-scope")?.as_str() {
                    "external" => false,
                    "internal" => true,
                    _ => {
                        return Err(PrivacyWalletBundleErrorV1::at("ironwood-scope"));
                    }
                };
            let diversifier_index = take_u32(
                &mut change.0,
                "diversifier_index",
                "ironwood-diversifier-index",
            )?;
            let value = u64::try_from(take_decimal_u128(
                &mut change.0,
                "value_decimal",
                "ironwood-change-value",
                false,
            )?)
            .map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-change-value"))?;
            let memo = take_hex(&mut change.0, "memo_hex", "ironwood-memo", true)?;
            let incoming = iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::incoming_viewing_key_v1(&spending_key, internal_scope).map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-change"))?;
            let recipient = iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::recipient_from_incoming_viewing_key_v1(&incoming, diversifier_index).map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-change"))?;
            let mut approved = value_object(approved, &["recipient_hex", "value_decimal", "memo_hex"], "ironwood-approved-change")?;
            let approved_recipient: [u8; 43] = take_hex(&mut approved, "recipient_hex", "ironwood-approved-change-recipient", false)?;
            let approved_value = take_decimal_u128(&mut approved, "value_decimal", "ironwood-approved-change-value", false)?;
            let approved_memo: [u8; 512] = take_hex(&mut approved, "memo_hex", "ironwood-approved-change-memo", true)?;
            if recipient != approved_recipient || u128::from(value) != approved_value || memo != approved_memo {
                return Err(PrivacyWalletBundleErrorV1::at("ironwood-change-approval"));
            }
            iroha_core_privacy::privacy_engines::orchard::ironwood_wallet::output_from_wallet_parts_v1(recipient, value, &memo).map_err(|_| PrivacyWalletBundleErrorV1::at("ironwood-change"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    outputs.extend(changes);
    Ok(PrivacyNativeActionRequestV1::Ironwood(
        crate::privacy_native_actions::IronwoodNoteActionRequestV1 {
            asset_definition_id,
            public_balance_scope,
            pool_id,
            anchor,
            anchor_epoch,
            expiry_height,
            spends,
            outputs,
            public_debit,
            minimum_action_count,
        },
    ))
}
