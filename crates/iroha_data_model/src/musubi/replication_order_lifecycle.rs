/// Immutable consensus binding from one `SoraFS` replication order to a complete Musubi archive.
///
/// This binding is installed atomically with the replication order, before any provider
/// completion or bundle-verification attestation exists. Providers can therefore authenticate the
/// exact archive commitment without trusting a publisher-supplied location request.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiReplicationOrderArchiveBindingV1 {
    /// Exact replication-order key duplicated for snapshot consistency validation.
    pub replication_order: ReplicationOrderId,
    /// Exact derived archive identity.
    pub archive_id: ArchiveId,
    /// Complete immutable archive commitment copied from authoritative registry state.
    pub commitment: MusubiArchiveCommitmentV1,
}

impl MusubiReplicationOrderArchiveBindingV1 {
    /// Construct an immutable replication-order/archive binding.
    #[must_use]
    pub const fn new(
        replication_order: ReplicationOrderId,
        archive_id: ArchiveId,
        commitment: MusubiArchiveCommitmentV1,
    ) -> Self {
        Self {
            replication_order,
            archive_id,
            commitment,
        }
    }

    /// Validate the order identity, complete commitment, derived archive identity, and wire bound.
    ///
    /// # Errors
    ///
    /// Returns an error if an identity is inert, the commitment is invalid, its derived identity
    /// differs, or the canonical binding exceeds the V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.commitment.validate()?;
        if digest_is_zero(self.replication_order.as_bytes()) || self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi replication-order/archive binding contains an inert identity",
            ));
        }
        if self.archive_id != self.commitment.archive_id() {
            return Err(ParseError::new(
                "Musubi replication-order/archive binding does not match its commitment",
            ));
        }
        if self.encode().len() > MUSUBI_MAX_REPLICATION_ORDER_ARCHIVE_BINDING_CANONICAL_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi replication-order/archive binding exceeds its canonical byte bound",
            ));
        }
        Ok(())
    }
}

/// Historical location facts retained when a replication order is permanently consumed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiRetiredReplicationOrderLocationV1 {
    /// Location identity the order formerly backed.
    pub location: MusubiArchiveLocationKeyV1,
    /// Exact completed provider set that justified that historical location admission.
    pub providers: Vec<ProviderId>,
}

impl MusubiRetiredReplicationOrderLocationV1 {
    /// Construct a permanent replication-order location tombstone.
    #[must_use]
    pub fn new(location: MusubiArchiveLocationKeyV1, providers: Vec<ProviderId>) -> Self {
        Self {
            location,
            providers,
        }
    }

    /// Validate the historical location identity and exact bounded provider set.
    ///
    /// # Errors
    ///
    /// Returns an error for inert location/provider identities, an empty or oversized provider
    /// set, or providers that are not strictly ordered and unique.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.location.archive_id.is_zero() || self.location.location_id.is_zero() {
            return Err(ParseError::new(
                "Musubi retired replication-order location identity is inert",
            ));
        }
        if self.providers.is_empty()
            || self.providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || self
                .providers
                .iter()
                .any(|provider| digest_is_zero(provider.as_bytes()))
            || self.providers.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi retired replication-order provider set is invalid",
            ));
        }
        Ok(())
    }
}

/// Lifecycle of one immutable replication-order/archive binding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiReplicationOrderLocationLifecycleV1 {
    /// The order is bound to the archive before any location has been admitted.
    PreLocation,
    /// The order is the current replication proof for this location.
    Active(MusubiArchiveLocationKeyV1),
    /// The order is permanently consumed with its exact historical provider set.
    Retired(MusubiRetiredReplicationOrderLocationV1),
}

/// Canonical consensus projection of a replication-order/archive binding and location lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiReplicationOrderLocationReferenceV1 {
    /// Immutable order-to-archive trust binding.
    pub binding: MusubiReplicationOrderArchiveBindingV1,
    /// Current use of the permanently bound order.
    pub lifecycle: MusubiReplicationOrderLocationLifecycleV1,
}

impl MusubiReplicationOrderLocationReferenceV1 {
    /// Construct a binding before location admission.
    #[must_use]
    pub const fn pre_location(binding: MusubiReplicationOrderArchiveBindingV1) -> Self {
        Self {
            binding,
            lifecycle: MusubiReplicationOrderLocationLifecycleV1::PreLocation,
        }
    }

    /// Return the active location, if this order currently backs one.
    #[must_use]
    pub const fn active_location(&self) -> Option<MusubiArchiveLocationKeyV1> {
        match &self.lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::Active(location) => Some(*location),
            MusubiReplicationOrderLocationLifecycleV1::PreLocation
            | MusubiReplicationOrderLocationLifecycleV1::Retired(_) => None,
        }
    }

    /// Return the consumed location, if this order has become a tombstone.
    #[must_use]
    pub const fn retired_location(&self) -> Option<MusubiArchiveLocationKeyV1> {
        match &self.lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::Retired(retired) => Some(retired.location),
            MusubiReplicationOrderLocationLifecycleV1::PreLocation
            | MusubiReplicationOrderLocationLifecycleV1::Active(_) => None,
        }
    }

    /// Validate the immutable trust binding and any location identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the binding is invalid or an active/retired location targets a
    /// different archive.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.binding.validate()?;
        let location = match &self.lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::PreLocation => return Ok(()),
            MusubiReplicationOrderLocationLifecycleV1::Active(location) => *location,
            MusubiReplicationOrderLocationLifecycleV1::Retired(retired) => {
                retired.validate()?;
                retired.location
            }
        };
        if location.archive_id != self.binding.archive_id || location.location_id.is_zero() {
            return Err(ParseError::new(
                "Musubi replication-order lifecycle targets an invalid archive location",
            ));
        }
        Ok(())
    }
}
