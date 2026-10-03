//! Original decode-budget provenance at a synchronous canonical admission boundary.

use super::{ActiveDecodeBudgetLayer, DecodeBudgetCounters, DecodeResourceError, Error};
use std::{cell::RefCell, sync::Arc};

#[derive(Clone)]
struct Observer {
    boundary: usize,
    protocol_start: usize,
    family: Option<Arc<DecodeBudgetCounters>>,
}

thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

struct Scope {
    previous: Option<Observer>,
}

impl Scope {
    fn enter() -> Self {
        let boundary = super::DECODE_BUDGET_LAYERS.with(|layers| layers.borrow().len());
        let previous = OBSERVER.with(|slot| {
            let previous = slot.borrow_mut().take();
            *slot.borrow_mut() = Some(Observer {
                boundary,
                protocol_start: previous
                    .as_ref()
                    .map_or(boundary, |value| value.protocol_start),
                family: previous.as_ref().and_then(|value| value.family.clone()),
            });
            previous
        });
        Self { previous }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        OBSERVER.with(|slot| {
            let current = slot.borrow_mut().take();
            if let Some(previous) = self.previous.as_mut()
                && previous.family.is_none()
            {
                previous.family = current.and_then(|value| value.family);
            }
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

/// Error origin established by one completed canonical decode attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeAttemptErrorKind {
    /// An actual native allocation failed; no allocation-pool release owner exists here.
    Allocator,
    /// The original attempt's enclosing admission budget refused the operation.
    EnclosingLimit,
    /// Invalid bytes, a protocol limit, or an error without current-attempt provenance.
    Invalid,
}

/// A canonical decoder error with its original non-wire resource classification.
#[derive(Debug)]
pub struct DecodeAttemptError {
    error: Error,
    kind: DecodeAttemptErrorKind,
}

impl DecodeAttemptError {
    /// Return the origin established before the decoder's observer scope unwound.
    #[must_use]
    pub const fn kind(&self) -> DecodeAttemptErrorKind {
        self.kind
    }

    /// Return the original error, retaining any opaque scope provenance for an outer attempt.
    #[must_use]
    pub fn into_error(self) -> Error {
        self.error
    }
}

impl std::fmt::Display for DecodeAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, formatter)
    }
}

impl std::error::Error for DecodeAttemptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Exact resource refusal emitted by a budget layer during canonical admission.
///
/// Construction is private to the budget owner. Reconstructing [`Error`] from
/// [`DecodeResourceError`] does not reconstruct this original scope identity.
#[derive(Clone, Debug)]
pub struct ScopedDecodeResourceError {
    resource: DecodeResourceError,
    family: Arc<DecodeBudgetCounters>,
    layer: usize,
    protocol_start: usize,
}

impl PartialEq for ScopedDecodeResourceError {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
            && Arc::ptr_eq(&self.family, &other.family)
            && self.layer == other.layer
            && self.protocol_start == other.protocol_start
    }
}

impl Eq for ScopedDecodeResourceError {}

impl ScopedDecodeResourceError {
    pub(super) const fn resource(&self) -> DecodeResourceError {
        self.resource
    }

    pub(super) fn matches_enclosing_scope(&self) -> bool {
        self.layer < self.protocol_start
            && OBSERVER.with(|slot| {
                slot.borrow().as_ref().is_some_and(|observer| {
                    observer
                        .family
                        .as_ref()
                        .is_some_and(|family| Arc::ptr_eq(family, &self.family))
                        && self.layer < observer.boundary
                })
            })
    }
}

impl std::fmt::Display for ScopedDecodeResourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.resource, formatter)
    }
}

impl std::error::Error for ScopedDecodeResourceError {}

pub(super) fn note_fresh_budget(counters: &Arc<DecodeBudgetCounters>) {
    OBSERVER.with(|slot| {
        if let Some(observer) = slot.borrow_mut().as_mut()
            && observer.family.is_none()
        {
            observer.family = Some(Arc::clone(counters));
        }
    });
}

pub(super) fn layers_in_order(
    layers: &[ActiveDecodeBudgetLayer],
) -> impl Iterator<Item = (usize, &ActiveDecodeBudgetLayer)> {
    let split = OBSERVER.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |observer| observer.protocol_start.min(layers.len()))
    });
    layers
        .iter()
        .enumerate()
        .skip(split)
        .chain(layers.iter().enumerate().take(split))
}

pub(super) fn budget_error(layer: usize, error: Error) -> Error {
    let Some(resource) = error.decode_resource_error() else {
        return error;
    };
    OBSERVER.with(|slot| {
        let observer = slot.borrow();
        let Some(observer) = observer.as_ref() else {
            return error;
        };
        let Some(family) = observer.family.as_ref() else {
            return error;
        };
        Error::ScopedDecodeResource(ScopedDecodeResourceError {
            resource,
            family: Arc::clone(family),
            layer,
            protocol_start: observer.protocol_start,
        })
    })
}

pub(crate) fn classify_decode_attempt<T>(
    decode: impl FnOnce() -> Result<T, Error>,
) -> Result<T, DecodeAttemptError> {
    let _scope = Scope::enter();
    decode().map_err(|error| {
        let kind = match &error {
            Error::AllocationFailed { .. } => DecodeAttemptErrorKind::Allocator,
            Error::ScopedDecodeResource(origin) => OBSERVER.with(|slot| {
                let observer = slot.borrow();
                let observer = observer
                    .as_ref()
                    .expect("original observer remains in scope");
                if observer
                    .family
                    .as_ref()
                    .is_some_and(|family| Arc::ptr_eq(family, &origin.family))
                    && origin.layer < observer.boundary
                    && origin.layer < origin.protocol_start
                {
                    DecodeAttemptErrorKind::EnclosingLimit
                } else {
                    DecodeAttemptErrorKind::Invalid
                }
            }),
            _ => DecodeAttemptErrorKind::Invalid,
        };
        DecodeAttemptError { error, kind }
    })
}

#[cfg(test)]
mod tests;
