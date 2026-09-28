//! Exact inline result custody for EBR writer admission.

use super::{EbrCellOwned, EbrCellWriterAcquisition, EbrCellWriterAdmissionError};

/// Success and refusal both retain the original acquired physical writer.
pub(super) type CloneAdmissionResult<'a, T, Charge, E> = Result<
    (
        EbrCellWriterAcquisition<'a, T, Charge>,
        EbrCellOwned<T, Charge>,
    ),
    (
        EbrCellWriterAcquisition<'a, T, Charge>,
        EbrCellWriterAdmissionError<E>,
    ),
>;
