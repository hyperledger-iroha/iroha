//! Independent generic State proof verification for Python using a selected schema and certified Global execution.
use crate::privacy_native_actions::network_id_from_genesis_hash_bytes;
use crate::*;
use iroha_data_model::sumeragi_finality::{
    KeyedStateRowsV1, MAX_STATE_PROOF_KEY_BYTES_V1, MAX_STATE_PROOF_ROWS_V1,
    MAX_STATE_PROOF_TABLE_BYTES_V1, MAX_STATE_ROWS_FRAME_BYTES_V1, SelectedStateSchemaV1,
    SumeragiFinalityCheckpoint, SumeragiFinalityProof, VerifiedFinalityPage,
    keyed_state::StateKeyV1, verify_checkpoint_page,
};
const MAX: usize = 16 * 1024 * 1024;

pub(super) fn bytes(text: &str, max: usize) -> Result<Vec<u8>, NativeValidationError> {
    if text.is_empty()
        || text.len() > max * 2
        || text.len() % 2 != 0
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(NativeValidationError::Value(
            "noncanonical or oversized State proof hex".into(),
        ));
    }
    hex::decode(text).map_err(|_| NativeValidationError::Value("malformed State proof hex".into()))
}
pub(super) fn fixed(text: &str) -> Result<[u8; 32], NativeValidationError> {
    bytes(text, 32)?
        .try_into()
        .map_err(|_| NativeValidationError::Value("State proof identity must be 32 bytes".into()))
}
pub(super) fn invalid() -> NativeValidationError {
    NativeValidationError::Value("State proof authentication failed".into())
}

pub(super) fn finality_page(
    checkpoint_hex: &str,
    proof_frames_hex: &[String],
    network_id_hex: &str,
) -> Result<VerifiedFinalityPage, NativeValidationError> {
    if proof_frames_hex.is_empty() || proof_frames_hex.len() > 64 {
        return Err(invalid());
    }
    let network_bytes = fixed(network_id_hex)?;
    if network_bytes[31] & 1 == 0 {
        return Err(invalid());
    }
    let network = network_id_from_genesis_hash_bytes(network_bytes);
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes(checkpoint_hex, MAX)?)
        .map_err(|_| invalid())?;
    let mut total = 0usize;
    let mut frames = Vec::new();
    for frame in proof_frames_hex {
        let encoded = bytes(frame, MAX)?;
        total = total
            .checked_add(encoded.len())
            .filter(|n| *n <= MAX)
            .ok_or_else(invalid)?;
        frames.push(
            norito::decode_canonical_for_admission::<SumeragiFinalityProof>(
                &encoded,
                norito::canonical_decode_limits(encoded.len()),
            )
            .map_err(|_| invalid())?,
        );
    }
    let page =
        verify_checkpoint_page(network, &checkpoint, &frames, 64, MAX).map_err(|_| invalid())?;
    page.tip()
        .verify_global_scope(network, checkpoint.chain_id())
        .map_err(|_| invalid())?;
    Ok(page)
}

/// Checked table and cell schemas bound to an independently selected deployment digest.
#[pyclass(frozen, name = "StateProofSchemaV1")]
pub(super) struct PyStateProofSchema(pub(crate) SelectedStateSchemaV1);

#[pymethods]
impl PyStateProofSchema {
    #[new]
    pub(super) fn new(
        py: Python<'_>,
        schema_bundle: &[u8],
        expected_schema_hash_hex: &str,
    ) -> PyResult<Self> {
        py.detach(|| {
            SelectedStateSchemaV1::from_bundle(schema_bundle, fixed(expected_schema_hash_hex)?)
                .map(Self)
                .map_err(|e| NativeValidationError::Value(e.to_string()))
        })
        .map_err(NativeValidationError::into_py)
    }
}

fn keys(expected: &[u8]) -> Result<Vec<StateKeyV1>, NativeValidationError> {
    if expected.len() > 64 * 1024 {
        return Err(invalid());
    }
    let rows: norito::json::Value = norito::json::from_slice(expected).map_err(|_| invalid())?;
    let rows = rows
        .as_array()
        .filter(|xs| !xs.is_empty() && xs.len() <= MAX_STATE_PROOF_ROWS_V1)
        .ok_or_else(invalid)?;
    rows.iter()
        .map(|row| {
            let object = row
                .as_object()
                .filter(|o| o.len() == 2)
                .ok_or_else(invalid)?;
            let table = object
                .get("table")
                .and_then(norito::json::Value::as_str)
                .filter(|s| !s.is_empty() && s.len() <= MAX_STATE_PROOF_TABLE_BYTES_V1)
                .ok_or_else(invalid)?;
            let key = object
                .get("key_hex")
                .and_then(norito::json::Value::as_str)
                .ok_or_else(invalid)?;
            let key = if key.is_empty() {
                Vec::new()
            } else {
                bytes(key, MAX_STATE_PROOF_KEY_BYTES_V1)?
            };
            Ok(StateKeyV1 {
                table: table.to_owned(),
                key,
            })
        })
        .collect()
}

/// Verify presence or absence for every exact requested row, in the caller-selected order.
#[pyfunction]
#[pyo3(name = "verify_state_rows_v1")]
pub(super) fn verify_state_rows_v1_py(
    py: Python<'_>,
    proof: &[u8],
    expected_rows_json: &[u8],
    schema: &PyStateProofSchema,
    checkpoint_hex: &str,
    proof_frames_hex: Vec<String>,
    network_id_hex: &str,
    height: u64,
) -> PyResult<Vec<Option<Py<PyBytes>>>> {
    let rows = py
        .detach(|| {
            if proof.is_empty() || proof.len() > MAX_STATE_ROWS_FRAME_BYTES_V1 || height < 2 {
                return Err(invalid());
            }
            let requested = keys(expected_rows_json)?;
            let page = finality_page(checkpoint_hex, &proof_frames_hex, network_id_hex)?;
            if page.tip().height() != height {
                return Err(invalid());
            }
            let proof: KeyedStateRowsV1 = norito::decode_canonical_for_admission(
                proof,
                norito::canonical_decode_limits(proof.len()),
            )
            .map_err(|_| invalid())?;
            proof
                .verify(page.tip(), &schema.0, &requested)
                .map_err(|e| NativeValidationError::Value(e.to_string()))?;
            Ok(proof.rows)
        })
        .map_err(NativeValidationError::into_py)?;
    Ok(rows
        .into_iter()
        .map(|row| row.value.map(|v| PyBytes::new(py, &v).unbind()))
        .collect())
}
