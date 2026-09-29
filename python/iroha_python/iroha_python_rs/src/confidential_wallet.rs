//! Owned Python wallet proving through the canonical Core relation selector.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use iroha_core_zk::{
    ProofRelation,
    confidential::{
        ConfidentialProof, ConfidentialProver, ConfidentialProverError, ConfidentialTree,
    },
    confidential_v2::{
        CONFIDENTIAL_TREE_DEPTH_V2, ConfidentialMerklePathV2, ConfidentialTransferInputV2,
        ConfidentialTransferOutputV2, ConfidentialUnshieldInputV2, ConfidentialUnshieldOutputV3,
    },
};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyList, PyString, PyTuple},
};
use zeroize::Zeroizing;

use super::{AssetDefinitionId, PyNetworkId, confidential_bytes_list_py};

pyo3::create_exception!(_crypto, ConfidentialWalletError, PyRuntimeError);

fn invalid(message: &'static str) -> PyErr {
    ConfidentialWalletError::new_err(("invalid_input", message))
}

fn native_error(error: ConfidentialProverError) -> PyErr {
    use ConfidentialProverError as E;
    let code = match &error {
        E::InvalidSpendKey => "invalid_spend_key",
        E::InputCount => "input_count",
        E::TreeCapacity => "tree_capacity",
        E::PathCount => "path_count",
        E::InvalidPath => "invalid_path",
        E::InputIndex => "input_index",
        E::PathIndexMismatch => "path_index_mismatch",
        E::DuplicateInput => "duplicate_input",
        E::OutputCount => "output_count",
        E::InvalidTransferAmounts => "invalid_transfer_amounts",
        E::InvalidInputAmounts => "invalid_input_amounts",
        E::InvalidPublicAmount => "invalid_public_amount",
        E::InvalidChange => "invalid_change",
        E::KeyPreparation(_) => "key_preparation",
        E::Proving(_) => "proving",
    };
    ConfidentialWalletError::new_err((code, error.to_string()))
}

// Borrow Python's bytes directly, with no intermediate heap copy. Python-owned
// immutable objects and interpreter-created copies remain outside our erasure contract.
fn word(value: &Bound<'_, PyAny>) -> PyResult<[u8; 32]> {
    value
        .cast::<PyBytes>()
        .map_err(|_| invalid("note, key and tree words must be bytes"))?
        .as_bytes()
        .try_into()
        .map_err(|_| invalid("note, key and tree words must contain exactly 32 bytes"))
}

/// Decode a primitive's private input into its clearing allocation directly.
pub(super) fn secret_bytes(
    value: &Bound<'_, PyAny>,
    maximum: usize,
) -> PyResult<Zeroizing<Vec<u8>>> {
    if let Ok(bytes) = value.cast::<PyBytes>() {
        let bytes = bytes.as_bytes();
        if bytes.len() > maximum {
            return Err(PyValueError::new_err(
                "private input exceeds its byte limit",
            ));
        }
        let mut result = Zeroizing::new(vec![0; bytes.len()]);
        result.copy_from_slice(bytes);
        return Ok(result);
    }
    if let Ok(text) = value.cast::<PyString>() {
        if !value.is_exact_instance_of::<PyString>()
            || text.len()? > maximum.saturating_mul(2).saturating_add(2)
        {
            return Err(PyValueError::new_err(
                "private hex input has invalid length or type",
            ));
        }
        // abi3-py39 may require an owned UTF-8 conversion. Give that conversion
        // its clearing owner before any fallible decode, too.
        return match text
            .to_cow()
            .map_err(|_| PyValueError::new_err("private input must contain hexadecimal bytes"))?
        {
            Cow::Borrowed(text) => secret_hex(text, maximum),
            Cow::Owned(text) => secret_hex(&Zeroizing::new(text), maximum),
        };
    }
    Err(PyTypeError::new_err(
        "private input must be bytes or a hex string",
    ))
}

fn secret_hex(text: &str, maximum: usize) -> PyResult<Zeroizing<Vec<u8>>> {
    let text = text.trim();
    let text = text.strip_prefix("0x").unwrap_or(text);
    if text.len() % 2 != 0 || text.len() / 2 > maximum {
        return Err(PyValueError::new_err(
            "private hex input has invalid length",
        ));
    }
    let mut result = Zeroizing::new(vec![0; text.len() / 2]);
    hex::decode_to_slice(text, result.as_mut_slice())
        .map_err(|_| PyValueError::new_err("private input must contain hexadecimal bytes"))?;
    Ok(result)
}

/// Decode one fixed private word while clearing even a rejected partial input.
pub(super) fn secret_word(value: &Bound<'_, PyAny>) -> PyResult<Zeroizing<[u8; 32]>> {
    let bytes = secret_bytes(value, 32)?;
    if bytes.len() != 32 {
        return Err(PyValueError::new_err(
            "private word must contain exactly 32 bytes",
        ));
    }
    let mut result = Zeroizing::new([0; 32]);
    result.copy_from_slice(&bytes);
    Ok(result)
}

fn items<'py>(value: &Bound<'py, PyAny>, maximum: usize) -> PyResult<Vec<Bound<'py, PyAny>>> {
    // Check the concrete container before copying even its reference array.
    if let Ok(list) = value.cast::<PyList>() {
        if list.len() <= maximum {
            return Ok(list.iter().collect());
        }
    } else if let Ok(tuple) = value.cast::<PyTuple>() {
        if tuple.len() <= maximum {
            return Ok(tuple.iter().collect());
        }
    } else {
        return Err(invalid("notes and tree evidence must be a list or tuple"));
    }
    Err(invalid(
        "notes or tree evidence exceed the fixed circuit capacity",
    ))
}

fn member<'py>(dict: &Bound<'py, PyDict>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    dict.get_item(name)?
        .ok_or_else(|| invalid("note or path is missing a required canonical field"))
}

fn amount(value: &Bound<'_, PyAny>) -> PyResult<u128> {
    if value.is_instance_of::<PyBool>() {
        return Err(invalid(
            "amount must be an integer from 0 through 2^128 - 1",
        ));
    }
    value
        .extract::<u128>()
        .map_err(|_| invalid("amount must be an integer from 0 through 2^128 - 1"))
}

/// Parse a canonical note into a clearing owner before reading private fields.
pub(super) fn input(value: &Bound<'_, PyAny>) -> PyResult<ConfidentialTransferInputV2> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid("input must be a dictionary"))?;
    if dict.contains("diversifier_hex")? || dict.contains("diversifierHex")? {
        return Err(invalid(
            "input requires canonical diversifier; aliases are rejected",
        ));
    }
    // Establish the clearing owner before the first private field or later fallible read.
    let mut note = ConfidentialTransferInputV2 {
        amount: 0,
        rho: [0; 32],
        diversifier: [0; 32],
        leaf_index: 0,
    };
    note.amount = amount(&member(dict, "amount")?)?;
    note.rho = word(&member(dict, "rho")?)?;
    note.diversifier = word(&member(dict, "diversifier")?)?;
    let index = member(dict, "leaf_index")?;
    if index.is_instance_of::<PyBool>() {
        return Err(invalid("leaf_index must be an unsigned integer"));
    }
    note.leaf_index = index
        .extract()
        .map_err(|_| invalid("leaf_index must be an unsigned integer"))?;
    Ok(note)
}

fn output(value: &Bound<'_, PyAny>) -> PyResult<ConfidentialTransferOutputV2> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid("output must be a dictionary"))?;
    let mut note = ConfidentialTransferOutputV2 {
        amount: 0,
        rho: [0; 32],
        owner_tag: [0; 32],
    };
    note.amount = amount(&member(dict, "amount")?)?;
    note.rho = word(&member(dict, "rho")?)?;
    note.owner_tag = word(&member(dict, "owner_tag")?)?;
    Ok(note)
}

fn change(value: &Bound<'_, PyAny>) -> PyResult<ConfidentialUnshieldOutputV3> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid("change must be a dictionary"))?;
    let mut note = ConfidentialUnshieldOutputV3 {
        amount: 0,
        rho: [0; 32],
    };
    note.amount = amount(&member(dict, "amount")?)?;
    note.rho = word(&member(dict, "rho")?)?;
    Ok(note)
}

fn path(value: &Bound<'_, PyAny>) -> PyResult<ConfidentialMerklePathV2> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid("path must be a dictionary"))?;
    let mut path = ConfidentialMerklePathV2 {
        root: [0; 32],
        siblings: Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2),
        directions: Vec::with_capacity(CONFIDENTIAL_TREE_DEPTH_V2),
        witness_nodes: Vec::new(),
    };
    path.root = word(&member(dict, "root")?)?;
    for item in items(&member(dict, "siblings")?, CONFIDENTIAL_TREE_DEPTH_V2)? {
        path.siblings.push(word(&item)?);
    }
    for item in items(&member(dict, "directions")?, CONFIDENTIAL_TREE_DEPTH_V2)? {
        if item.is_instance_of::<PyBool>() {
            return Err(invalid("path directions must be integer 0 or 1"));
        }
        let direction = item
            .extract::<u8>()
            .map_err(|_| invalid("path directions must be integer 0 or 1"))?;
        if direction > 1 {
            return Err(invalid("path directions must be integer 0 or 1"));
        }
        path.directions.push(direction);
    }
    // Intermediate witness nodes are computed by Core, never supplied as a second witness.
    Ok(path)
}

enum Tree {
    Commitments {
        root: [u8; 32],
        leaves: Vec<[u8; 32]>,
    },
    Paths {
        root: [u8; 32],
        paths: Vec<ConfidentialMerklePathV2>,
    },
}

impl Tree {
    fn parse(
        root: &Bound<'_, PyAny>,
        leaves: Option<&Bound<'_, PyAny>>,
        paths: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let root = word(root)?;
        match (leaves, paths) {
            (Some(leaves), None) => Ok(Self::Commitments {
                root,
                leaves: items(leaves, 1 << CONFIDENTIAL_TREE_DEPTH_V2)?
                    .iter()
                    .map(word)
                    .collect::<PyResult<_>>()?,
            }),
            (None, Some(paths)) => Ok(Self::Paths {
                root,
                paths: items(paths, 2)?.iter().map(path).collect::<PyResult<_>>()?,
            }),
            _ => Err(invalid(
                "supply either complete tree_commitments or one input_path per actual note",
            )),
        }
    }

    fn borrowed(&self) -> ConfidentialTree<'_> {
        match self {
            Self::Commitments { root, leaves } => ConfidentialTree::Commitments {
                root: *root,
                leaves,
            },
            Self::Paths { root, paths } => ConfidentialTree::Paths { root: *root, paths },
        }
    }
}

fn envelope(py: Python<'_>, proof: ConfidentialProof) -> PyResult<Py<PyDict>> {
    let result = PyDict::new(py);
    let relation = match proof.relation {
        ProofRelation::ConfidentialTransfer => "transfer",
        ProofRelation::ConfidentialFullUnshield => "full_redemption",
        ProofRelation::ConfidentialChangeUnshield => "redemption_with_change",
        _ => {
            return Err(ConfidentialWalletError::new_err((
                "internal",
                "unexpected wallet proof relation",
            )));
        }
    };
    result.set_item("relation", relation)?;
    result.set_item("backend", proof.proof.backend)?;
    result.set_item("proof", PyBytes::new(py, &proof.proof.bytes))?;
    result.set_item("root", PyBytes::new(py, &proof.root))?;
    result.set_item(
        "nullifiers",
        confidential_bytes_list_py(py, &proof.nullifiers)?,
    )?;
    result.set_item(
        "output_commitments",
        confidential_bytes_list_py(py, &proof.output_commitments)?,
    )?;
    Ok(result.unbind())
}

/// Own one clearing Core key; in-flight detached work retains its own lifetime.
#[pyclass(frozen, name = "ConfidentialProver", module = "iroha_native._crypto")]
pub(crate) struct PyConfidentialProver {
    owner: Mutex<Option<Arc<ConfidentialProver>>>,
}

impl PyConfidentialProver {
    fn acquire(&self) -> PyResult<Arc<ConfidentialProver>> {
        self.owner
            .lock()
            .map_err(|_| ConfidentialWalletError::new_err(("internal", "wallet ownership failed")))?
            .as_ref()
            .cloned()
            .ok_or_else(|| {
                ConfidentialWalletError::new_err(("closed", "confidential prover is closed"))
            })
    }
}

#[pymethods]
impl PyConfidentialProver {
    #[new]
    fn new(
        network_id: &PyNetworkId,
        asset_definition_id: &str,
        spend_key: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let key = Zeroizing::new(word(spend_key)?);
        let asset = asset_definition_id
            .parse::<AssetDefinitionId>()
            .map_err(|_| invalid("asset_definition_id must be a canonical asset identifier"))?;
        if asset.to_string() != asset_definition_id {
            return Err(invalid(
                "asset_definition_id must be a canonical asset identifier",
            ));
        }
        let prover =
            ConfidentialProver::new(*network_id.as_inner(), &asset, key).map_err(native_error)?;
        Ok(Self {
            owner: Mutex::new(Some(Arc::new(prover))),
        })
    }

    /// Close future operations; already detached work finishes with its owned key.
    fn close(&self) -> PyResult<()> {
        self.owner
            .lock()
            .map_err(|_| ConfidentialWalletError::new_err(("internal", "wallet ownership failed")))?
            .take();
        Ok(())
    }

    fn __repr__(&self) -> &'static str {
        "ConfidentialProver(private_context=[REDACTED])"
    }

    #[pyo3(signature = (*, root, inputs, outputs, tree_commitments=None, input_paths=None))]
    fn prove_transfer(
        &self,
        py: Python<'_>,
        root: &Bound<'_, PyAny>,
        inputs: &Bound<'_, PyAny>,
        outputs: &Bound<'_, PyAny>,
        tree_commitments: Option<&Bound<'_, PyAny>>,
        input_paths: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let prover = self.acquire()?;
        let inputs = items(inputs, 2)?
            .iter()
            .map(input)
            .collect::<PyResult<Vec<_>>>()?;
        let outputs = items(outputs, 2)?
            .iter()
            .map(output)
            .collect::<PyResult<Vec<_>>>()?;
        if inputs.is_empty() {
            return Err(native_error(ConfidentialProverError::InputCount));
        }
        if outputs.is_empty() {
            return Err(native_error(ConfidentialProverError::OutputCount));
        }
        let tree = Tree::parse(root, tree_commitments, input_paths)?;
        let proof = py
            .detach(move || prover.prove_transfer(tree.borrowed(), inputs, outputs))
            .map_err(native_error)?;
        envelope(py, proof)
    }

    #[pyo3(signature = (*, root, inputs, public_amount, change_note=None, tree_commitments=None, input_paths=None))]
    fn prove_unshield(
        &self,
        py: Python<'_>,
        root: &Bound<'_, PyAny>,
        inputs: &Bound<'_, PyAny>,
        public_amount: &Bound<'_, PyAny>,
        change_note: Option<&Bound<'_, PyAny>>,
        tree_commitments: Option<&Bound<'_, PyAny>>,
        input_paths: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let prover = self.acquire()?;
        let inputs = items(inputs, 2)?
            .iter()
            .map(input)
            .map(|note| {
                note.map(|note| ConfidentialUnshieldInputV2 {
                    amount: note.amount,
                    rho: note.rho,
                    diversifier: note.diversifier,
                    leaf_index: note.leaf_index,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        if inputs.is_empty() {
            return Err(native_error(ConfidentialProverError::InputCount));
        }
        let public_amount = amount(public_amount)?;
        let change = change_note.map(change).transpose()?;
        let tree = Tree::parse(root, tree_commitments, input_paths)?;
        let proof = py
            .detach(move || prover.prove_unshield(tree.borrowed(), inputs, public_amount, change))
            .map_err(native_error)?;
        envelope(py, proof)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyConfidentialProver>()?;
    module.add(
        "ConfidentialWalletError",
        module.py().get_type::<ConfidentialWalletError>(),
    )
}

#[cfg(test)]
mod tests;
