//! Ironwood SDK selectors and semantic verification over the generic keyed State transport.
use super::*;
use iroha_core_privacy::ironwood_state_rows::{
    IronwoodPoolContextV1, IronwoodStateSchemaV1, ironwood_pool_rows,
};
use iroha_data_model::sumeragi_finality::{KeyedStateRowsV1, keyed_state::StateKeyV1};

/// Checked selection and codec identities reused across every request for an installed schema.
#[pyclass(frozen, name = "IronwoodStateSchemaV1")]
pub(crate) struct PyIronwoodStateSchema(pub(crate) IronwoodStateSchemaV1);

#[pymethods]
impl PyIronwoodStateSchema {
    #[new]
    fn new(py: Python<'_>, schema_bundle: &[u8], expected_schema_hash_hex: &str) -> PyResult<Self> {
        py.detach(|| {
            IronwoodStateSchemaV1::from_bundle(schema_bundle, fixed(expected_schema_hash_hex)?)
                .map(Self)
                .map_err(|e| NativeValidationError::Value(e.to_string()))
        })
        .map_err(NativeValidationError::into_py)
    }
}

/// One verified Global finality prefix for a single live SDK read. It is immutable,
/// never serialized, and constructed only by verification against the caller's checkpoint.
#[pyclass(frozen, name = "_IronwoodFinalityPage")]
pub(crate) struct PyFinalityPage(VerifiedFinalityPage);

#[pymethods]
impl PyFinalityPage {
    #[new]
    pub(crate) fn new(
        py: Python<'_>,
        checkpoint_hex: &str,
        proof_frames_hex: Vec<String>,
        network_id_hex: &str,
        height: u64,
    ) -> PyResult<Self> {
        py.detach(|| {
            let page = finality_page(checkpoint_hex, &proof_frames_hex, network_id_hex)?;
            if page.tip().height() != height {
                return Err(invalid());
            }
            Ok(Self(page))
        })
        .map_err(NativeValidationError::into_py)
    }

    /// Finish a live read against this same verified page.
    #[expect(
        clippy::too_many_arguments,
        reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
    )]
    fn verify_rows(
        &self,
        py: Python<'_>,
        pool_rows: &[u8],
        balance_rows: &[u8],
        #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
        pool_id_hex: &str,
        asset_definition_id: &str,
        payer_account_id: &str,
        expected_profile_digest_hex: &str,
        expected_schema: &PyIronwoodStateSchema,
    ) -> PyResult<String> {
        py.detach(|| {
            verify_rows_at_page(
                &self.0,
                pool_rows,
                balance_rows,
                chain_discriminant,
                pool_id_hex,
                asset_definition_id,
                payer_account_id,
                expected_profile_digest_hex,
                expected_schema,
            )
        })
        .map_err(NativeValidationError::into_py)
    }
}

fn decode_rows(frame: &[u8]) -> Result<KeyedStateRowsV1, NativeValidationError> {
    if frame.is_empty() || frame.len() > MAX {
        return Err(invalid());
    }
    norito::decode_canonical_for_admission(frame, norito::canonical_decode_limits(frame.len()))
        .map_err(|_| invalid())
}

fn pool_context<'a>(
    proof: &KeyedStateRowsV1,
    page: &'a VerifiedFinalityPage,
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_schema: &'a PyIronwoodStateSchema,
) -> Result<
    (
        IronwoodPoolContextV1<'a>,
        iroha_data_model::asset::AssetDefinitionId,
        AccountId,
    ),
    NativeValidationError,
> {
    let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let asset: iroha_data_model::asset::AssetDefinitionId =
        asset_definition_id.parse().map_err(|_| invalid())?;
    if asset.to_string() != asset_definition_id {
        return Err(invalid());
    }
    let payer =
        parse_selected_account_id_native(payer_account_id, "payer_account_id", chain_discriminant)?;
    let pool = PrivacyPoolIdV1::new(fixed(pool_id_hex)?);
    let context =
        IronwoodPoolContextV1::verify(page.tip(), &expected_schema.0, pool, &asset, &payer, proof)
            .map_err(|_| invalid())?;
    Ok((context, asset, payer))
}

fn request_json(rows: &[StateKeyV1]) -> Result<String, NativeValidationError> {
    let rows: Vec<_> = rows.iter().map(|position| norito::json!({"table": (position.table.as_str()), "key_hex": (hex::encode(&position.key))})).collect();
    norito::json::to_json(&norito::json!({"rows": rows})).map_err(|_| invalid())
}

/// Encode the two generic row selectors derived from an independently chosen pool.
#[pyfunction]
#[pyo3(name = "ironwood_pool_rows_request_v1")]
pub(crate) fn ironwood_pool_rows_request_v1_py(pool_id_hex: &str) -> PyResult<String> {
    let generate = || {
        request_json(
            &ironwood_pool_rows(PrivacyPoolIdV1::new(fixed(pool_id_hex)?))
                .map_err(|_| invalid())?,
        )
    };
    generate().map_err(NativeValidationError::into_py)
}

/// Authenticate pool and head against the trusted checkpoint before exposing dependent selectors.
#[pyfunction]
#[pyo3(name = "ironwood_balance_rows_request_v1")]
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
pub(crate) fn ironwood_balance_rows_request_v1_py(
    py: Python<'_>,
    pool_rows: &[u8],
    verified: &PyFinalityPage,
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_schema: &PyIronwoodStateSchema,
) -> PyResult<(String, PyIronwoodReadPlan)> {
    py.detach(move || {
        let proof = decode_rows(pool_rows)?;
        let page = &verified.0;
        if page
            .tip()
            .commitment()
            .schedule
            .current
            .network_id
            .as_bytes()
            != &fixed(network_id_hex)?
        {
            return Err(invalid());
        }
        let (context, _, _) = pool_context(
            &proof,
            page,
            chain_discriminant,
            pool_id_hex,
            asset_definition_id,
            payer_account_id,
            expected_schema,
        )?;
        let details = context.remaining_rows().map_err(|_| invalid())?;
        let request = request_json(&details)?;
        Ok((
            request,
            PyIronwoodReadPlan::new(
                network_id_hex,
                chain_discriminant,
                pool_id_hex,
                asset_definition_id,
                payer_account_id,
                expected_schema,
                details,
            )?,
        ))
    })
    .map_err(NativeValidationError::into_py)
}

/// Derived only from an authenticated pool state and root head. Contains exact keys,
/// never balance values, signing material or an assertion about a later block.
#[pyclass(frozen, name = "IronwoodReadPlan")]
pub(crate) struct PyIronwoodReadPlan {
    network: [u8; 32],
    chain: u16,
    pool: [u8; 32],
    asset: String,
    payer: String,
    schema: [u8; 32],
    rows: Vec<StateKeyV1>,
}

impl PyIronwoodReadPlan {
    fn new(
        network: &str,
        chain: u16,
        pool: &str,
        asset: &str,
        payer: &str,
        schema: &PyIronwoodStateSchema,
        details: Vec<StateKeyV1>,
    ) -> Result<Self, NativeValidationError> {
        let pool = fixed(pool)?;
        let mut rows = ironwood_pool_rows(PrivacyPoolIdV1::new(pool)).map_err(|_| invalid())?;
        rows.extend(details);
        rows.sort();
        Ok(Self {
            network: fixed(network)?,
            chain,
            pool,
            asset: asset.into(),
            payer: payer.into(),
            schema: schema.0.selected().digest(),
            rows,
        })
    }
    fn check(
        &self,
        network: &str,
        chain: u16,
        pool: &str,
        asset: &str,
        payer: &str,
        schema: &PyIronwoodStateSchema,
    ) -> Result<(), NativeValidationError> {
        if self.network != fixed(network)?
            || self.chain != chain
            || self.pool != fixed(pool)?
            || self.asset != asset
            || self.payer != payer
            || self.schema != schema.0.selected().digest()
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[pymethods]
impl PyIronwoodReadPlan {
    /// Derive a single batch only when the plan matches all identity and schema pins.
    fn request(
        &self,
        network: &str,
        #[pyo3(from_py_with = exact_chain_discriminant)] chain: u16,
        pool: &str,
        asset: &str,
        payer: &str,
        schema: &PyIronwoodStateSchema,
    ) -> PyResult<String> {
        self.check(network, chain, pool, asset, payer, schema)
            .and_then(|()| request_json(&self.rows))
            .map_err(NativeValidationError::into_py)
    }
}

fn subset(
    base: &KeyedStateRowsV1,
    extra: Option<&KeyedStateRowsV1>,
    keys: &[StateKeyV1],
) -> Result<KeyedStateRowsV1, NativeValidationError> {
    let rows = keys
        .iter()
        .map(|key| {
            base.rows
                .iter()
                .chain(extra.into_iter().flat_map(|proof| proof.rows.iter()))
                .find(|row| &row.position == key)
                .cloned()
                .ok_or_else(invalid)
        })
        .collect::<Result<_, _>>()?;
    Ok(KeyedStateRowsV1 {
        height: base.height,
        block_hash: base.block_hash,
        schema_hash: base.schema_hash,
        rows,
    })
}

type PyWarmRead = (PyIronwoodReadPlan, String, Py<PyBytes>, Option<Py<PyBytes>>);

struct WarmRead {
    plan: PyIronwoodReadPlan,
    missing: String,
    pool: Vec<u8>,
    detail: Option<Vec<u8>>,
}

/// Normalize only verified rows into the same canonical two proof batches used for recovery.
/// The SDK also retains the original combined response and any supplementary response.
#[pyfunction]
#[pyo3(name="ironwood_warm_rows_v1", signature=(plan, primary, supplement, verified, network_id_hex, chain_discriminant, pool_id_hex, asset_definition_id, payer_account_id, expected_schema))]
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
pub(crate) fn ironwood_warm_rows_v1_py(
    py: Python<'_>,
    plan: &PyIronwoodReadPlan,
    primary: &[u8],
    supplement: Option<&[u8]>,
    verified: &PyFinalityPage,
    network_id_hex: &str,
    #[pyo3(from_py_with=exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_schema: &PyIronwoodStateSchema,
) -> PyResult<PyWarmRead> {
    let read = py
        .detach(move || {
            plan.check(
                network_id_hex,
                chain_discriminant,
                pool_id_hex,
                asset_definition_id,
                payer_account_id,
                expected_schema,
            )?;
            if primary
                .len()
                .saturating_add(supplement.map_or(0, <[u8]>::len))
                > MAX
            {
                return Err(invalid());
            }
            let page = &verified.0;
            if page
                .tip()
                .commitment()
                .schedule
                .current
                .network_id
                .as_bytes()
                != &fixed(network_id_hex)?
            {
                return Err(invalid());
            }
            let bulk = decode_rows(primary)?;
            bulk.verify(page.tip(), expected_schema.0.selected(), &plan.rows)
                .map_err(|_| invalid())?;
            let prefix = subset(
                &bulk,
                None,
                &ironwood_pool_rows(PrivacyPoolIdV1::new(fixed(pool_id_hex)?))
                    .map_err(|_| invalid())?,
            )?;
            let (pool, _, _) = pool_context(
                &prefix,
                page,
                chain_discriminant,
                pool_id_hex,
                asset_definition_id,
                payer_account_id,
                expected_schema,
            )?;
            let detail = pool.remaining_rows().map_err(|_| invalid())?;
            let missing: Vec<_> = detail
                .iter()
                .filter(|key| plan.rows.binary_search(key).is_err())
                .cloned()
                .collect();
            let current_plan = PyIronwoodReadPlan::new(
                network_id_hex,
                chain_discriminant,
                pool_id_hex,
                asset_definition_id,
                payer_account_id,
                expected_schema,
                detail.clone(),
            )?;
            let extra = supplement.map(decode_rows).transpose()?;
            if let Some(ref proof) = extra {
                proof
                    .verify(page.tip(), expected_schema.0.selected(), &missing)
                    .map_err(|_| invalid())?;
            }
            let complete = if missing.is_empty() || extra.is_some() {
                let proof = subset(&bulk, extra.as_ref(), &detail)?;
                pool.verify_balances(&proof).map_err(|_| invalid())?;
                Some(norito::encode_canonical(&proof).map_err(|_| invalid())?)
            } else {
                None
            };
            Ok(WarmRead {
                plan: current_plan,
                missing: request_json(&missing)?,
                pool: norito::encode_canonical(&prefix).map_err(|_| invalid())?,
                detail: complete,
            })
        })
        .map_err(NativeValidationError::into_py)?;
    Ok((
        read.plan,
        read.missing,
        PyBytes::new(py, &read.pool).unbind(),
        read.detail.map(|wire| PyBytes::new(py, &wire).unbind()),
    ))
}

/// Verify the pool and balance row witnesses against an independently retained checkpoint and consecutive finality frames.
#[pyfunction]
#[pyo3(name = "verify_ironwood_state_rows_v1")]
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
pub(crate) fn verify_ironwood_state_rows_v1_py(
    py: Python<'_>,
    pool_rows: &[u8],
    balance_rows: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_schema: &PyIronwoodStateSchema,
) -> PyResult<String> {
    // PyO3 keeps the immutable bytes/strings alive for this call. Only Rust views
    // and owned frame strings cross this boundary; errors become Python values
    // after reattachment. Keep decoding, finality, cut and signature checks detached.
    py.detach(move || {
        verify_ironwood_state_rows_v1(
            pool_rows,
            balance_rows,
            checkpoint_hex,
            proof_frames_hex,
            network_id_hex,
            chain_discriminant,
            pool_id_hex,
            asset_definition_id,
            payer_account_id,
            expected_profile_digest_hex,
            expected_schema,
        )
    })
    .map_err(NativeValidationError::into_py)
}
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
fn verify_ironwood_state_rows_v1(
    pool_rows: &[u8],
    balance_rows: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    network_id_hex: &str,
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_schema: &PyIronwoodStateSchema,
) -> Result<String, NativeValidationError> {
    if pool_rows.is_empty()
        || balance_rows.is_empty()
        || pool_rows.len().saturating_add(balance_rows.len()) > MAX
    {
        return Err(invalid());
    }
    let page = finality_page(checkpoint_hex, &proof_frames_hex, network_id_hex)?;
    verify_rows_at_page(
        &page,
        pool_rows,
        balance_rows,
        chain_discriminant,
        pool_id_hex,
        asset_definition_id,
        payer_account_id,
        expected_profile_digest_hex,
        expected_schema,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
fn verify_rows_at_page(
    page: &VerifiedFinalityPage,
    pool_rows: &[u8],
    balance_rows: &[u8],
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_schema: &PyIronwoodStateSchema,
) -> Result<String, NativeValidationError> {
    if pool_rows.len().saturating_add(balance_rows.len()) > MAX {
        return Err(invalid());
    }
    let _guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let initial = decode_rows(pool_rows)?;
    let (context, asset, payer) = pool_context(
        &initial,
        page,
        chain_discriminant,
        pool_id_hex,
        asset_definition_id,
        payer_account_id,
        expected_schema,
    )?;
    let (payer_balance, reserve_balance) = context
        .verify_balances(&decode_rows(balance_rows)?)
        .map_err(|_| invalid())?;
    let state = context.state();
    verified_context_json(
        page,
        pool_id_hex,
        &asset,
        &payer,
        expected_profile_digest_hex,
        state,
        &payer_balance,
        &reserve_balance,
    )
}

/// Authenticate the committed public bridge effect using the retained preparation row witnesses.
/// No live-tip balance subtraction is used: unrelated transactions may change either account.
#[pyfunction]
#[pyo3(name = "verify_ironwood_payment_effect_from_rows_v1")]
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
pub(crate) fn verify_ironwood_payment_effect_from_rows_v1_py(
    py: Python<'_>,
    pool_rows: &[u8],
    balance_rows: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    payment_proof_frames_hex: Vec<String>,
    signed_transaction_versioned: &[u8],
    network_id_hex: &str,
    #[pyo3(from_py_with = exact_chain_discriminant)] chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_schema: &PyIronwoodStateSchema,
    expected_amount_decimal: &str,
) -> PyResult<String> {
    // PyO3 keeps the immutable bytes/strings alive for this call. Only Rust views
    // and owned frame strings cross this boundary; errors become Python values
    // after reattachment. Keep decoding, finality, cut and signature checks detached.
    py.detach(move || {
        verify_ironwood_payment_effect_from_rows_v1(
            pool_rows,
            balance_rows,
            checkpoint_hex,
            proof_frames_hex,
            payment_proof_frames_hex,
            signed_transaction_versioned,
            network_id_hex,
            chain_discriminant,
            pool_id_hex,
            asset_definition_id,
            payer_account_id,
            expected_profile_digest_hex,
            expected_schema,
            expected_amount_decimal,
        )
    })
    .map_err(NativeValidationError::into_py)
}
#[expect(
    clippy::too_many_arguments,
    reason = "Keep independently selected evidence and trust pins explicit across this verifier boundary."
)]
fn verify_ironwood_payment_effect_from_rows_v1(
    pool_rows: &[u8],
    balance_rows: &[u8],
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    payment_proof_frames_hex: Vec<String>,
    signed_transaction_versioned: &[u8],
    network_id_hex: &str,
    chain_discriminant: u16,
    pool_id_hex: &str,
    asset_definition_id: &str,
    payer_account_id: &str,
    expected_profile_digest_hex: &str,
    expected_schema: &PyIronwoodStateSchema,
    expected_amount_decimal: &str,
) -> Result<String, NativeValidationError> {
    let cut_json = verify_ironwood_state_rows_v1(
        pool_rows,
        balance_rows,
        checkpoint_hex,
        proof_frames_hex,
        network_id_hex,
        chain_discriminant,
        pool_id_hex,
        asset_definition_id,
        payer_account_id,
        expected_profile_digest_hex,
        expected_schema,
    )?;
    verify_payment_from_context(
        &cut_json,
        payment_proof_frames_hex,
        signed_transaction_versioned,
        network_id_hex,
        chain_discriminant,
        pool_id_hex,
        asset_definition_id,
        payer_account_id,
        expected_profile_digest_hex,
        expected_amount_decimal,
    )
}
