//! Bounded membership and complete-interval checks for digest-only witnesses.

use super::*;

impl NoritoKeyDigestRangeProofV1 {
    /// Verify a complete digest-only interval against an independently owned root.
    ///
    /// Verification authenticates value digests, not undisclosed value bytes.
    ///
    /// # Errors
    /// Rejects forged paths, omitted or reordered rows, wrong roots, and bounds.
    pub fn verify(
        &self,
        request: NoritoKeyRangeVerifyRequestV1<'_>,
    ) -> Result<VerifiedNoritoKeyDigestRangeV1<'_>, NoritoKeyRangeError> {
        let NoritoKeyRangeVerifyRequestV1 {
            expected_root,
            schema_hash,
            domain,
            start,
            end,
            max_rows,
            max_bytes,
        } = request;
        validate_domain(domain)?;
        validate_interval(start, end)?;
        validate_limits(max_rows, max_bytes)?;
        self.verify_geometry(max_rows, max_bytes)?;
        if self.entries == 0 {
            if self.before.is_some() || !self.rows.is_empty() || self.after.is_some() {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            if root_hash(0, schema_hash, domain, empty_hash()) != *expected_root {
                return Err(NoritoKeyRangeError::RootMismatch);
            }
            return Ok(VerifiedNoritoKeyDigestRangeV1 {
                rows: &self.rows,
                root: *expected_root,
                schema_hash: *schema_hash,
                domain_hash: Hash::new(domain),
            });
        }
        let mut next_index = 0_u32;
        let mut previous_key: Option<&[u8]> = if let Some(before) = &self.before {
            if before.key.as_slice() >= start {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            NoritoKeyRangeProofV1::verify_member(
                self.entries,
                before.index,
                &before.key,
                before.value_digest,
                &before.siblings,
                (expected_root, schema_hash, domain),
            )?;
            next_index = before.index + 1;
            Some(&before.key)
        } else {
            None
        };
        for row in &self.rows {
            if row.index != next_index
                || row.key.as_slice() < start
                || row.key.as_slice() >= end
                || previous_key.is_some_and(|key| key >= row.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            NoritoKeyRangeProofV1::verify_member(
                self.entries,
                row.index,
                &row.key,
                row.value_digest,
                &row.siblings,
                (expected_root, schema_hash, domain),
            )?;
            next_index += 1;
            previous_key = Some(&row.key);
        }
        if let Some(after) = &self.after {
            if after.index != next_index
                || after.key.as_slice() < end
                || previous_key.is_some_and(|key| key >= after.key.as_slice())
            {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            NoritoKeyRangeProofV1::verify_member(
                self.entries,
                after.index,
                &after.key,
                after.value_digest,
                &after.siblings,
                (expected_root, schema_hash, domain),
            )?;
        } else if next_index != self.entries {
            return Err(NoritoKeyRangeError::InvalidProof);
        }
        Ok(VerifiedNoritoKeyDigestRangeV1 {
            rows: &self.rows,
            root: *expected_root,
            schema_hash: *schema_hash,
            domain_hash: Hash::new(domain),
        })
    }

    /// Validate every row/path bound before checking any cryptographic membership.
    fn verify_geometry(
        &self,
        max_rows: usize,
        max_bytes: usize,
    ) -> Result<(), NoritoKeyRangeError> {
        if self.rows.len() > max_rows {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let path_depth = depth(self.entries)?;
        let path_bytes = path_depth
            .checked_mul(Hash::LENGTH)
            .ok_or(NoritoKeyRangeError::Capacity)?;
        let mut size = 0_usize;
        add_size(&mut size, PROOF_HEADER_BYTES, max_bytes)?;
        for row in &self.rows {
            if row.key.len() > MAX_NORITO_KEY_BYTES || row.siblings.len() != path_depth {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            add_size(
                &mut size,
                ROW_HEADER_BYTES + Hash::LENGTH + path_bytes,
                max_bytes,
            )?;
            add_size(&mut size, row.key.len(), max_bytes)?;
        }
        for boundary in [&self.before, &self.after].into_iter().flatten() {
            if boundary.key.len() > MAX_NORITO_KEY_BYTES || boundary.siblings.len() != path_depth {
                return Err(NoritoKeyRangeError::InvalidProof);
            }
            add_size(&mut size, BOUNDARY_HEADER_BYTES + path_bytes, max_bytes)?;
            add_size(&mut size, boundary.key.len(), max_bytes)?;
        }
        Ok(())
    }
}
