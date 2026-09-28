//! Automatically selected BN254 batches with charged native conversion storage.

use pyo3::{
    exceptions::{PyMemoryError, PyValueError},
    prelude::*,
    types::{PyList, PyListMethods, PySequence, PySequenceMethods, PyTuple},
};

type Batch = fn(&[[u64; 4]], &[[u64; 4]], &mut [[u64; 4]]) -> bool;

fn field_at(sequence: &Bound<'_, PySequence>, index: usize) -> PyResult<[u64; 4]> {
    let field = sequence.get_item(index)?.extract::<[u64; 4]>()?;
    if !ivm::bn254_vec::FieldElem(field).is_canonical() {
        return Err(PyValueError::new_err(
            "BN254 input must be below the field modulus",
        ));
    }
    Ok(field)
}

fn output_error(error: ivm::AccelerationOutputError) -> PyErr {
    match error {
        ivm::AccelerationOutputError::InvalidLayout => PyValueError::new_err(error.to_string()),
        ivm::AccelerationOutputError::Capacity | ivm::AccelerationOutputError::Allocation => {
            PyMemoryError::new_err(error.to_string())
        }
    }
}

fn batch(
    py: Python<'_>,
    lhs: &Bound<'_, PySequence>,
    rhs: &Bound<'_, PySequence>,
    operation: Batch,
) -> PyResult<Py<PyTuple>> {
    let count = lhs.len()?;
    if count != rhs.len()? {
        return Err(PyValueError::new_err(
            "BN254 batches must have equal lengths",
        ));
    }
    // Reject malformed rows before any native payload allocation. Each decoded
    // snapshot is charged as well as the output; Python owns the original rows.
    for index in 0..count {
        field_at(lhs, index)?;
        field_at(rhs, index)?;
    }
    let mut left = ivm::try_acceleration_output::<[u64; 4]>(count).map_err(output_error)?;
    let mut right = ivm::try_acceleration_output::<[u64; 4]>(count).map_err(output_error)?;
    let mut output = ivm::try_acceleration_output::<[u64; 4]>(count).map_err(output_error)?;
    for index in 0..count {
        // Revalidate while capturing in case an application-defined sequence
        // changed during conversion. No foreign result has been published yet.
        left[index] = field_at(lhs, index)?;
        right[index] = field_at(rhs, index)?;
    }
    if !operation(&left, &right, &mut output) {
        return Err(PyValueError::new_err("invalid BN254 batch operands"));
    }
    let result = PyList::empty(py);
    for field in output.as_slice() {
        result.append(PyTuple::new(py, field.iter().copied())?)?;
    }
    // Native input/output owners remain charged through the final foreign copy.
    Ok(result.to_tuple().unbind())
}

/// Add canonical BN254 rows with automatic CPU/GPU selection.
#[pyfunction]
pub(crate) fn bn254_add_many(
    py: Python<'_>,
    lhs: &Bound<'_, PySequence>,
    rhs: &Bound<'_, PySequence>,
) -> PyResult<Py<PyTuple>> {
    batch(py, lhs, rhs, ivm::bn254_vec::add_batch_into)
}

/// Subtract canonical BN254 rows with automatic CPU/GPU selection.
#[pyfunction]
pub(crate) fn bn254_sub_many(
    py: Python<'_>,
    lhs: &Bound<'_, PySequence>,
    rhs: &Bound<'_, PySequence>,
) -> PyResult<Py<PyTuple>> {
    batch(py, lhs, rhs, ivm::bn254_vec::sub_batch_into)
}

/// Multiply canonical BN254 rows with automatic CPU/GPU selection.
#[pyfunction]
pub(crate) fn bn254_mul_many(
    py: Python<'_>,
    lhs: &Bound<'_, PySequence>,
    rhs: &Bound<'_, PySequence>,
) -> PyResult<Py<PyTuple>> {
    batch(py, lhs, rhs, ivm::bn254_vec::mul_batch_into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_resource_refusal_is_not_reported_as_cuda_unavailability() {
        Python::initialize();
        Python::attach(|py| {
            assert!(
                output_error(ivm::AccelerationOutputError::InvalidLayout)
                    .is_instance_of::<PyValueError>(py)
            );
            for error in [
                ivm::AccelerationOutputError::Capacity,
                ivm::AccelerationOutputError::Allocation,
            ] {
                assert!(output_error(error).is_instance_of::<PyMemoryError>(py));
            }
        });
    }

    #[test]
    fn malformed_rows_and_lengths_fail_before_computation() {
        Python::initialize();
        Python::attach(|py| {
            let valid = PyList::new(py, [[1u64, 0, 0, 0]]).unwrap();
            let invalid = PyList::new(py, [ivm::bn254_vec::MODULUS]).unwrap();
            let empty = PyList::empty(py);
            let valid = valid.as_any().cast::<PySequence>().unwrap();
            let invalid = invalid.as_any().cast::<PySequence>().unwrap();
            let empty = empty.as_any().cast::<PySequence>().unwrap();
            let never: Batch = |_, _, _| panic!("malformed rows must not compute");
            for (left, right) in [(valid, empty), (valid, invalid), (invalid, valid)] {
                assert!(
                    batch(py, left, right, never)
                        .unwrap_err()
                        .is_instance_of::<PyValueError>(py)
                );
            }
        });
    }
}
