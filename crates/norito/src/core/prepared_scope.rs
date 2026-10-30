//! Physically prepaid counter controls reused across synchronous canonical decodes.

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation, ChargedShared,
    InsufficientReservation, PrepaidSharedError, SharedFromChargeError,
};

use super::*;

/// Local construction failure for the prepared decode scope, without protocol meaning.
#[derive(Debug, thiserror::Error)]
pub enum PreparedDecodeScopeError {
    /// The parent reservation belongs to a different original source.
    #[error("prepared decode scope reservation belongs to another pool")]
    ForeignPool,
    /// The unchanged original remainder cannot fund both actual controls.
    #[error(transparent)]
    Reservation(InsufficientReservation),
    /// An actual prepaid control allocation failed.
    #[error(transparent)]
    Allocation(PrepaidSharedError),
    /// This workspace has exhausted its non-repeating local attempt identities.
    #[error("prepared decode scope attempt identity is exhausted")]
    AttemptExhausted,
}

/// Two original charged counter controls reused by the canonical framed entry.
///
/// The controls represent the payload-derived and explicitly supplied protocol
/// limits. Enclosing caller scopes remain in the same borrowed active chain.
/// Reuse allocates no Arc, layer Vec, TLS backing, or replacement identity.
/// Captured errors retain the exact original control through its final reader.
/// This owner does not fund decoded values, input frames or alignment scratch.
/// It cannot be cloned, and each synchronous use requires an exclusive borrow.
pub struct PreparedDecodeWorkspace {
    derived: ChargedShared<DecodeBudgetCounters>,
    explicit: ChargedShared<DecodeBudgetCounters>,
    attempt: u64,
}

/// Retained construction of the two prepaid canonical decode counter controls.
///
/// The original charges exist before the first physical allocation. A later refusal
/// returns this same attempt, including every completed control and unused charge.
/// It grants no decoding scope until both controls have been constructed.
pub struct PreparedDecodeWorkspaceConstruction {
    controls: [Option<ChargedShared<DecodeBudgetCounters>>; 2],
    charges: [Option<AllocationCharge>; 2],
}
impl PreparedDecodeWorkspaceConstruction {
    /// Whether every original control and unfinished charge retains this exact pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.controls
            .iter()
            .flatten()
            .all(|control| control.belongs_to(budget))
            && self
                .charges
                .iter()
                .flatten()
                .all(|charge| charge.belongs_to(budget))
    }

    /// Retain the two original charges without allocating or reacquiring credit.
    ///
    /// # Errors
    /// A foreign charge returns both originals before any physical allocation.
    #[allow(
        clippy::result_large_err,
        reason = "return both original charges on refusal"
    )]
    pub fn from_charges(
        budget: &AllocationBudget,
        charges: [AllocationCharge; 2],
    ) -> Result<Self, ([AllocationCharge; 2], PreparedDecodeScopeError)> {
        if !charges.iter().all(|charge| charge.belongs_to(budget)) {
            return Err((charges, PreparedDecodeScopeError::ForeignPool));
        }
        Ok(Self {
            charges: charges.map(Some),
            controls: [None, None],
        })
    }

    /// Complete the original controls, retaining the same partial attempt on refusal.
    ///
    /// # Errors
    /// Returns the exact refused layout and unchanged charge together with every
    /// earlier control. Retrying never performs another pool admission.
    #[allow(
        clippy::result_large_err,
        reason = "return the exact partial controls and charges"
    )]
    pub fn try_finish(mut self) -> Result<PreparedDecodeWorkspace, (Self, SharedFromChargeError)> {
        for index in 0..self.controls.len() {
            if self.controls[index].is_some() {
                continue;
            }
            let charge = self.charges[index].take().expect("original counter charge");
            match ChargedShared::reserve_from_charge(charge) {
                Ok(control) => {
                    self.controls[index] =
                        Some(control.initialize(DecodeBudgetCounters::default()));
                }
                Err((charge, error)) => {
                    self.charges[index] = Some(charge);
                    return Err((self, error));
                }
            }
        }
        Ok(PreparedDecodeWorkspace {
            derived: self.controls[0].take().expect("completed derived counter"),
            explicit: self.controls[1].take().expect("completed explicit counter"),
            attempt: 0,
        })
    }
}

impl PreparedDecodeWorkspace {
    /// Exact physical counter/control layouts required before an attempt starts.
    #[must_use]
    pub fn allocation_layouts() -> [Layout; 2] {
        [ChargedShared::<DecodeBudgetCounters>::allocation_layout(); 2]
    }

    /// Construct both actual controls using only the original parent's remainder.
    ///
    /// # Errors
    /// Foreign source or a short aggregate remainder performs no allocation and
    /// leaves the parent unchanged. Physical failure retires completed controls
    /// and refunds only the charges split for these controls; unused parent
    /// credit remains with the original reservation.
    pub fn from_reservation(
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, PreparedDecodeScopeError> {
        if !reservation.belongs_to(budget) {
            return Err(PreparedDecodeScopeError::ForeignPool);
        }
        let required = Self::allocation_layouts()
            .iter()
            .map(Layout::size)
            .sum::<usize>();
        if reservation.remaining_bytes() < required {
            return Err(PreparedDecodeScopeError::Reservation(
                InsufficientReservation {
                    requested_bytes: required,
                    remaining_bytes: reservation.remaining_bytes(),
                },
            ));
        }
        let derived = ChargedShared::from_reservation(DecodeBudgetCounters::default(), reservation)
            .map_err(|(_, error)| PreparedDecodeScopeError::Allocation(error))?;
        let explicit =
            ChargedShared::from_reservation(DecodeBudgetCounters::default(), reservation)
                .map_err(|(_, error)| PreparedDecodeScopeError::Allocation(error))?;
        Ok(Self {
            derived,
            explicit,
            attempt: 0,
        })
    }

    /// Whether both physical controls retain the supplied original finite pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.derived.belongs_to(budget) && self.explicit.belongs_to(budget)
    }

    fn begin_attempt(&mut self) -> Result<(), PreparedDecodeScopeError> {
        let attempt = self
            .attempt
            .checked_add(1)
            .ok_or(PreparedDecodeScopeError::AttemptExhausted)?;
        self.attempt = attempt;
        for counters in [&self.derived, &self.explicit] {
            counters.total_elements.store(0, Ordering::Relaxed);
            counters.total_allocated_bytes.store(0, Ordering::Relaxed);
            counters.attempt.store(attempt, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Decode a complete canonical value using these same two original controls.
    ///
    /// Header policy, payload checks, field decoding and streaming canonical
    /// comparison use the ordinary canonical kernel. Header failure precedes
    /// workspace reuse. The default scope encloses the complete operation; the
    /// explicit schema scope encloses payload decoding, as in the ordinary entry.
    /// Different frame sizes, value types and limits may share these controls.
    /// Each use has a checked new attempt identity: retained errors keep their
    /// original classification, family and attempt, without acquiring authority
    /// from a later decode. No control allocation or pool admission occurs here.
    ///
    /// This owns only counter controls. Input frames, decoded graphs and custom
    /// serializer scratch still require their own physical allocation custody.
    ///
    /// # Errors
    /// Returns the unchanged canonical error with its original enclosing-limit
    /// provenance, or a local scope invariant when attempt identities exhaust.
    pub fn decode_canonical<T>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
    ) -> Result<T, crate::CanonicalDecodeControlError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        use crate::{CanonicalDecodeControlError, CanonicalDecodeScope};

        let header = classify_decode_attempt(|| crate::checked_canonical_header(bytes))
            .map_err(CanonicalDecodeControlError::Decode)?;
        self.begin_attempt()
            .map_err(CanonicalDecodeControlError::Scope)?;
        let depth = DECODE_NESTING_DEPTH.with(Cell::get);
        let context =
            |limits, counters: &ChargedShared<DecodeBudgetCounters>| DecodeBudgetContext {
                layer: ActiveDecodeBudgetLayer {
                    budget: DecodeBudgetLayer {
                        limits,
                        counters: CounterOwner::Prepared(counters.clone()),
                    },
                    base_depth: depth,
                },
                depth,
            };
        let defaults = context(crate::canonical_decode_limits(bytes.len()), &self.derived);
        let schema = context(limits, &self.explicit);
        let mut slot = TypedFieldSlot::<T>::new();
        classify_decode_attempt(|| {
            decode_attempt::note_fresh_budget(&defaults.layer.budget.counters);
            crate::decode_canonical_with_scopes_into_slot(
                bytes,
                header,
                &mut slot,
                CanonicalDecodeScope::Prepared(&defaults),
                CanonicalDecodeScope::Prepared(&schema),
            )?;
            // Keep the actual value in its slot until the observer retires.
            slot.value().map(|_| ())
        })
        .map_err(CanonicalDecodeControlError::Decode)?;
        Ok(crate::finish_canonical_admission(&mut slot, |value| value))
    }

    /// Run under the original derived/explicit protocol ceilings without allocating.
    ///
    /// This is synchronous scope custody, not a decoder or an authentication
    /// claim. The closure must finish its work before returning. The two scopes
    /// use the same enforcement kernel as ordinary and lazy decode contexts.
    /// Call inside the canonical admission classifier so an actual enclosing
    /// refusal is captured before scope retirement. A later use of this same
    /// workspace cannot make a retained old error belong to the new attempt.
    ///
    /// # Errors
    /// Returns a source-less local invariant error before entering a scope when
    /// the attempt counter cannot advance. No wraparound identity is permitted.
    pub fn with_limits<R>(
        &mut self,
        derived: DecodeLimits,
        explicit: DecodeLimits,
        body: impl FnOnce() -> R,
    ) -> Result<R, PreparedDecodeScopeError> {
        self.begin_attempt()?;
        let depth = DECODE_NESTING_DEPTH.with(Cell::get);
        let layers = [
            ActiveDecodeBudgetLayer {
                budget: DecodeBudgetLayer {
                    limits: derived,
                    counters: CounterOwner::Prepared(self.derived.clone()),
                },
                base_depth: depth,
            },
            ActiveDecodeBudgetLayer {
                budget: DecodeBudgetLayer {
                    limits: explicit,
                    counters: CounterOwner::Prepared(self.explicit.clone()),
                },
                base_depth: depth,
            },
        ];
        decode_attempt::note_fresh_budget(&layers[0].budget.counters);
        Ok(budget_scope::with_layers(&layers, depth, body))
    }
}

#[cfg(test)]
mod canonical_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod value_tests;

/// Original byte error, original destination refusal, or prepared scope invariant.
#[derive(Debug, thiserror::Error)]
pub enum PreparedDecodeError<E> {
    /// The canonical decoder's exact cause, classified before its scopes retired.
    #[error(transparent)]
    Codec(DecodeAttemptError),
    /// The exact prepared destination refusal; this is never malformed input.
    #[error("prepared destination refused: {0}")]
    Destination(#[source] E),
    /// Original prepared scope/control failure, without protocol meaning.
    #[error(transparent)]
    Scope(PreparedDecodeScopeError),
}

/// A prepared destination attempt with header-first original control admission.
#[derive(Debug, thiserror::Error)]
pub enum OwnedPreparedDecodeError<E> {
    /// The original finite pool refused both actual counter controls.
    #[error(transparent)]
    Admission(AllocationRefusal),
    /// The existing canonical, destination, or physical-control failure.
    #[error(transparent)]
    Decode(PreparedDecodeError<E>),
}

/// The real ordinary owning entry whose record bounds this prepared caller preserves.
#[derive(Clone, Copy)]
enum PreparedRecordRoot {
    CanonicalField,
    ArchiveView,
}

impl PreparedDecodeWorkspace {
    /// Decode into original leaf owners after checking the canonical header and
    /// funding both exact scope controls from their original finite pool.
    ///
    /// This is the same derived field, canonical framing and final streaming
    /// comparison kernel as [`Self::decode_canonical_into`]. Destinations own
    /// their actual graph allocations; this entry funds only the two controls.
    /// The caller retains the same workspace on success and failure; a late leaf
    /// refusal cannot refund its own controls or satisfy its own pool wait. Retry
    /// reuses those controls and preserves their checked per-attempt identities.
    /// A refusal resets validity without retiring any destination backing.
    ///
    /// # Errors
    /// Malformed canonical headers precede pool admission. Returns the original
    /// pool, allocator, enclosing scope or destination cause without formatting.
    pub fn decode_canonical_into_with_owned_controls<T, D>(
        bytes: &[u8],
        limits: DecodeLimits,
        budget: &AllocationBudget,
        workspace: &mut Option<Self>,
        destination: &mut D,
    ) -> Result<(), OwnedPreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeRecordFields<D>,
        D: PreparedRecordDestination<T>,
    {
        destination.reset();
        let result = (|| {
            let header = classify_decode_attempt(|| crate::checked_canonical_header(bytes))
                .map_err(|error| {
                    OwnedPreparedDecodeError::Decode(PreparedDecodeError::Codec(error))
                })?;
            if let Some(original) = workspace.as_ref() {
                if !original.belongs_to(budget) {
                    return Err(OwnedPreparedDecodeError::Decode(
                        PreparedDecodeError::Scope(PreparedDecodeScopeError::ForeignPool),
                    ));
                }
            } else {
                let mut reservation = budget
                    .try_reserve_layouts(Self::allocation_layouts())
                    .map_err(OwnedPreparedDecodeError::Admission)?;
                *workspace = Some(Self::from_reservation(budget, &mut reservation).map_err(
                    |error| OwnedPreparedDecodeError::Decode(PreparedDecodeError::Scope(error)),
                )?);
            }
            workspace
                .as_mut()
                .expect("original prepared controls are installed")
                .decode_canonical_record_with_root::<T, D>(
                    bytes,
                    limits,
                    destination,
                    PreparedRecordRoot::CanonicalField,
                    Some(header),
                )
                .map_err(OwnedPreparedDecodeError::Decode)
        })();
        if result.is_err() {
            destination.reset();
        }
        result
    }

    /// Fill one prepared destination using the sole derived field walk and
    /// authenticate its complete original canonical frame by streaming comparison.
    ///
    /// Header identity, flags, padding, checksum, field/depth ceilings and error
    /// ordering share the ordinary canonical decoder kernels. No alternate wire
    /// representation is accepted. The destination and this scope keep their
    /// original backing through failure, reset and retry; no owned decoder fallback
    /// or lazy alignment buffer is constructed. Individual destination leaves
    /// must use canonical prepared parsers rather than ordinary heap decoders.
    ///
    /// # Errors
    /// Retains exact scope provenance for an enclosing refusal, original leaf
    /// refusal for local custody failure, and intrinsic invalidity for malformed
    /// bytes. On every error the destination's validity is reset while preserving
    /// its allocated storage. The caller retains the original input and all I/O
    /// cursors; this entry consumes neither a secret nor an input descriptor.
    pub fn decode_canonical_into<T, D>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        destination: &mut D,
    ) -> Result<(), PreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeRecordFields<D>,
        D: PreparedRecordDestination<T>,
    {
        self.decode_canonical_into_with_root::<T, D>(
            bytes,
            limits,
            destination,
            PreparedRecordRoot::CanonicalField,
            None,
        )
    }

    /// Fill a canonical frame through its original archive-view record boundary.
    ///
    /// This is the prepared counterpart of [`ArchiveView::decode`] for a derived
    /// positional record with slice decoding. Both use payload-derived protocol
    /// ceilings and the same field walk, without treating the whole record as a
    /// canonical child field or adding a record depth before its first field.
    /// The field prefixes, leaf depth checks and inherited ceilings still apply.
    /// Header/schema/padding/checksum validation precedes the record scope as in
    /// the archive view. Exact canonical frame comparison follows complete filling.
    /// No archived backing, alignment scratch or ordinary owned fallback is made.
    /// The separate canonical-field entry retains its original root checks.
    ///
    /// # Errors
    /// Preserves original byte/resource or destination causes and resets validity
    /// on failure while retaining the original controls and destination storage.
    pub fn decode_canonical_archive_into<T, D>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        destination: &mut D,
    ) -> Result<(), PreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeRecordFields<D>,
        for<'de> T: NoritoDeserialize<'de> + DecodeFromSlice<'de>,
        D: PreparedRecordDestination<T>,
    {
        self.decode_canonical_into_with_root::<T, D>(
            bytes,
            limits,
            destination,
            PreparedRecordRoot::ArchiveView,
            None,
        )
    }

    /// Materialize a canonical value with its sole generated field traversal.
    ///
    /// This entry uses the same header, flags, depth, quota and comparison kernel
    /// as [`Self::decode_canonical_into`]. It returns the actual value only after
    /// its complete canonical frame matches the original bytes. If final
    /// consumption or streaming comparison fails, the exact value is restored
    /// into its original destination before validity is reset. No graph clone,
    /// owned-decoder fallback or replacement control is created.
    ///
    /// Consuming validation hooks retain their existing ownership contract: a
    /// hook that consumes and rejects a value cannot restore it. Concrete graph
    /// producers must audit that path as well as their field/default allocations.
    ///
    /// # Errors
    /// Preserves original codec, enclosing resource, destination and scope
    /// causes. Reset must retain storage; it cannot replenish inherited work.
    pub fn decode_canonical_value_into<T, D>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        destination: &mut D,
    ) -> Result<T, PreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeValueFields<D>,
        D: PreparedValueDestination<T>,
    {
        destination.reset();
        let result = self.decode_canonical_with_root::<T, _, _>(
            bytes,
            limits,
            PreparedRecordRoot::CanonicalField,
            None,
            |payload| {
                let (value, used) = T::decode_value_fields(payload, destination)?;
                let checked = validate_decode_consumption(payload, used)
                    .and_then(|()| compare_prepared_frame::<T, T>(bytes, &value));
                if let Err(error) = checked {
                    destination.restore_value(value);
                    return Err(DecodeIntoError::Codec(error));
                }
                Ok(value)
            },
        );
        if result.is_err() {
            destination.reset();
        }
        result
    }

    fn decode_canonical_into_with_root<T, D>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        destination: &mut D,
        root: PreparedRecordRoot,
        header: Option<Header>,
    ) -> Result<(), PreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeRecordFields<D>,
        D: PreparedRecordDestination<T>,
    {
        destination.reset();
        let result = self.decode_canonical_record_with_root::<T, D>(
            bytes,
            limits,
            destination,
            root,
            header,
        );
        if result.is_err() {
            destination.reset();
        }
        result
    }

    fn decode_canonical_record_with_root<T, D>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        destination: &mut D,
        root: PreparedRecordRoot,
        header: Option<Header>,
    ) -> Result<(), PreparedDecodeError<D::Error>>
    where
        T: NoritoSerialize + DecodeRecordFields<D>,
        D: PreparedRecordDestination<T>,
    {
        self.decode_canonical_with_root::<T, _, _>(bytes, limits, root, header, |payload| {
            let (_, used) = T::decode_fields(payload, destination)?;
            validate_decode_consumption(payload, used)?;
            compare_prepared_frame::<T, D>(bytes, destination)?;
            Ok(())
        })
    }

    fn decode_canonical_with_root<T, R, E>(
        &mut self,
        bytes: &[u8],
        limits: DecodeLimits,
        root: PreparedRecordRoot,
        header: Option<Header>,
        decode: impl FnOnce(&[u8]) -> Result<R, DecodeIntoError<E>>,
    ) -> Result<R, PreparedDecodeError<E>>
    where
        T: NoritoSerialize,
    {
        decode_attempt::observe(|| {
            let header = match header {
                Some(header) => header,
                None => crate::checked_canonical_header(bytes)
                    .map_err(|error| PreparedDecodeError::Codec(decode_attempt::capture(error)))?,
            };
            // ArchiveView authenticates its complete frame and type before entering
            // the payload-derived scope. Preserve that ordering without constructing
            // an archived payload or running a second decoder.
            let archive = match root {
                PreparedRecordRoot::CanonicalField => None,
                PreparedRecordRoot::ArchiveView => {
                    let view = from_bytes_view(bytes).map_err(|error| {
                        PreparedDecodeError::Codec(decode_attempt::capture(error))
                    })?;
                    if view.schema() != crate::schema::identity::frame_hash::<T>() {
                        return Err(PreparedDecodeError::Codec(decode_attempt::capture(
                            Error::SchemaMismatch,
                        )));
                    }
                    if view.padding_len != payload_alignment_padding_for::<T>() {
                        return Err(PreparedDecodeError::Codec(decode_attempt::capture(
                            Error::LengthMismatch,
                        )));
                    }
                    Some(view)
                }
            };
            let derived_length = archive
                .as_ref()
                .map_or(bytes.len(), |view| view.as_bytes().len());
            let attempt = self.with_limits(
                crate::canonical_decode_limits(derived_length),
                limits,
                || {
                    let _canonical_flags = DecodeFlagsGuard::enter(default_encode_flags());
                    let _outer_context = PayloadCtxGuard::enter(bytes);
                    let payload = match &archive {
                        Some(view) => view.as_bytes(),
                        None => crate::checked_uncompressed_payload::<T>(bytes, &header)?,
                    };
                    let _flags = DecodeFlagsGuard::enter(header.flags);
                    let _depth = match root {
                        PreparedRecordRoot::CanonicalField => {
                            check_decode_field_length(
                                u64::try_from(payload.len()).map_err(|_| Error::LengthMismatch)?,
                            )?;
                            Some(DecodeDepthGuard::enter()?)
                        }
                        PreparedRecordRoot::ArchiveView => None,
                    };
                    let _context = match root {
                        PreparedRecordRoot::CanonicalField => PayloadCtxGuard::enter(payload),
                        PreparedRecordRoot::ArchiveView => {
                            PayloadCtxGuard::enter_with_schema_and_flags(
                                payload,
                                header.schema,
                                header.flags,
                            )
                        }
                    };
                    let _boundary = FieldDecodeBoundaryGuard::enter(match root {
                        PreparedRecordRoot::CanonicalField => FieldDecodeBoundary::Canonical,
                        PreparedRecordRoot::ArchiveView => FieldDecodeBoundary::Prefix,
                    });
                    decode(payload)
                },
            );
            match attempt {
                Err(error) => Err(PreparedDecodeError::Scope(error)),
                Ok(Err(DecodeIntoError::Destination(error))) => {
                    Err(PreparedDecodeError::Destination(error))
                }
                Ok(Err(DecodeIntoError::Codec(error))) => {
                    let error = match error {
                        Error::DecodeFlagsMismatch { .. } => Error::NonCanonicalEncoding,
                        Error::UnsupportedCompression { found, .. }
                            if found == Compression::Zstd as u8 =>
                        {
                            Error::NonCanonicalEncoding
                        }
                        error => error,
                    };
                    Err(PreparedDecodeError::Codec(decode_attempt::capture(error)))
                }
                Ok(Ok(value)) => Ok(value),
            }
        })
    }
}

/// Compare through the existing typed writer, with no alternate frame layout.
fn compare_prepared_frame<T: NoritoSerialize, D: SerializePayload>(
    bytes: &[u8],
    value: &D,
) -> Result<(), Error> {
    let mut exact = ExactSliceWriter::new(bytes);
    let encoded = encode_frames::write_typed_payload_frame::<T, D, _>(
        value,
        &mut exact,
        default_encode_flags(),
    );
    if exact.mismatched() {
        return Err(Error::NonCanonicalEncoding);
    }
    encoded?;
    if !exact.is_complete() {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(())
}
