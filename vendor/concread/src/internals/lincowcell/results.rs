//! Exact inline refusal owners for original linear-cell writer acquisition.

use super::{
    LinCowCellOwned, LinCowCellOwnedAcquisition, LinCowCellWriteTxn, LinCowCellWriterAcquisition,
    OwnedWriteError, WriterAdmissionError,
};

/// Refusal retains the original acquired writer before any successor exists.
pub(super) type WriterAdmissionResult<'a, T, R, U, Charge, E> = Result<
    LinCowCellWriteTxn<'a, T, R, U, Charge>,
    (
        LinCowCellWriterAcquisition<'a, T, R, U, Charge>,
        WriterAdmissionError<E>,
    ),
>;
/// Refusal returns the original unpublished owner without allocating a wrapper.
pub(super) type OwnedAcquireResult<'a, T, R, U, Charge> = Result<
    LinCowCellOwnedAcquisition<'a, T, R, U, Charge>,
    (LinCowCellOwned<T, R, U, Charge>, OwnedWriteError),
>;
/// Refusal returns the original unpublished owner after unlocking acquisition.
pub(super) type OwnedWriteResult<'a, T, R, U, Charge> = Result<
    LinCowCellWriteTxn<'a, T, R, U, Charge>,
    (LinCowCellOwned<T, R, U, Charge>, OwnedWriteError),
>;
