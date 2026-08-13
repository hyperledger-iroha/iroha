impl MusubiRegistryPolicyV1 {
    /// Validate version, bounds, ordering, and mode-specific allowlist use.
    ///
    /// # Errors
    ///
    /// Returns an error if pricing is invalid, the version or revision is invalid, the allowlist
    /// is oversized or noncanonical, or a non-allowlisted mode carries entries.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.alias_pricing.validate()?;
        if self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.revision == 0
            || self.allowlisted_dataspaces.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .allowlisted_dataspaces
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || (!matches!(self.mode, MusubiRegistryAdmissionModeV1::Allowlisted)
                && !self.allowlisted_dataspaces.is_empty())
        {
            return Err(ParseError::new(
                "Musubi registry policy is invalid or noncanonical",
            ));
        }
        Ok(())
    }
    /// Validate a strict first-release transition from `current`.
    ///
    /// # Errors
    ///
    /// Returns an error if either policy is invalid, revision arithmetic overflows, the policy is
    /// not the exact successor, or pricing changes do not use the required pricing revision.
    pub fn validate_successor(&self, current: &Self) -> Result<(), ParseError> {
        current.validate()?;
        self.validate()?;
        let expected_revision = current
            .revision
            .checked_add(1)
            .ok_or_else(|| ParseError::new("Musubi registry policy revision overflow"))?;
        if self.revision != expected_revision {
            return Err(ParseError::new(
                "Musubi replacement policy revision must be the exact successor",
            ));
        }
        let prices_changed = self.alias_pricing.length_1_xor != current.alias_pricing.length_1_xor
            || self.alias_pricing.length_2_xor != current.alias_pricing.length_2_xor
            || self.alias_pricing.length_3_xor != current.alias_pricing.length_3_xor
            || self.alias_pricing.length_4_xor != current.alias_pricing.length_4_xor
            || self.alias_pricing.length_5_to_32_xor != current.alias_pricing.length_5_to_32_xor;
        if prices_changed {
            let expected_pricing_revision = current
                .alias_pricing
                .revision
                .checked_add(1)
                .ok_or_else(|| ParseError::new("Musubi alias pricing revision overflow"))?;
            if self.alias_pricing.revision != expected_pricing_revision {
                return Err(ParseError::new(
                    "changed Musubi alias prices require the exact successor pricing revision",
                ));
            }
        } else if self.alias_pricing != current.alias_pricing {
            return Err(ParseError::new(
                "unchanged Musubi alias prices must retain the current pricing policy",
            ));
        }
        Ok(())
    }
}
