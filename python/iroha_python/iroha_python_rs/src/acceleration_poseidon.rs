//! Automatic Poseidon batches with charged native conversion storage.

use pyo3::{
    exceptions::{PyMemoryError, PyValueError},
    prelude::*,
    types::{PyList, PyListMethods, PySequence, PySequenceMethods, PyTuple},
};
fn output_error(error: ivm::AccelerationOutputError) -> PyErr {
    match error {
        ivm::AccelerationOutputError::InvalidLayout => PyValueError::new_err(error.to_string()),
        _ => PyMemoryError::new_err(error.to_string()),
    }
}
fn batch<T: Copy + Default>(
    py: Python<'_>,
    inputs: &Bound<'_, PySequence>,
    row: impl Fn(usize) -> PyResult<T>,
    operation: fn(&[T], &mut [u64]) -> bool,
) -> PyResult<Py<PyTuple>> {
    let count = inputs.len()?;
    for index in 0..count {
        row(index)?;
    }
    let mut snapshot = ivm::try_acceleration_output::<T>(count).map_err(output_error)?;
    let mut output = ivm::try_acceleration_output::<u64>(count).map_err(output_error)?;
    for (index, destination) in snapshot.iter_mut().enumerate() {
        *destination = row(index)?;
    }
    if !operation(&snapshot, &mut output) {
        return Err(PyValueError::new_err("invalid Poseidon batch geometry"));
    }
    let result = PyList::empty(py);
    for &value in output.as_slice() {
        result.append(value)?;
    }
    Ok(result.to_tuple().unbind())
}
/// Compute two-word Poseidon rows with automatic CPU/GPU selection.
#[pyfunction]
pub(crate) fn poseidon2_many(
    py: Python<'_>,
    inputs: &Bound<'_, PySequence>,
) -> PyResult<Py<PyTuple>> {
    batch(
        py,
        inputs,
        |index| inputs.get_item(index)?.extract::<(u64, u64)>(),
        ivm::poseidon2_many_into,
    )
}
/// Compute six-word Poseidon rows with automatic CPU/GPU selection.
#[pyfunction]
pub(crate) fn poseidon6_many(
    py: Python<'_>,
    inputs: &Bound<'_, PySequence>,
) -> PyResult<Py<PyTuple>> {
    batch(
        py,
        inputs,
        |index| inputs.get_item(index)?.extract::<[u64; 6]>(),
        ivm::poseidon6_many_into,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusal_uses_typed_foreign_errors() {
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
    fn malformed_rows_do_not_invoke_computation() {
        Python::initialize();
        Python::attach(|py| {
            let rows = PyList::new(py, [[1u64, 2, 3]]).unwrap();
            let rows = rows.as_any().cast::<PySequence>().unwrap();
            let result = batch(
                py,
                rows,
                |index| rows.get_item(index)?.extract::<(u64, u64)>(),
                |_, _| panic!("malformed rows must fail before compute"),
            );
            assert!(result.is_err());
        });
    }
}
