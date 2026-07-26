    fn validate_android_key_description(
        desc: &KeyDescription,
        meta: &AndroidMetadata,
        challenge: &ReceiptChallenge,
    ) -> Result<(), InstructionExecutionError> {
        if desc.attestation_challenge.len() != challenge.iroha_bytes.len()
            || desc.attestation_challenge != challenge.iroha_bytes
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation challenge mismatch".into(),
            )
            .into());
        }
        if desc.attestation_security_level == SecurityLevel::Software
            || desc.keymaster_security_level == SecurityLevel::Software
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must not use software-only security".into(),
            )
            .into());
        }
        if meta.require_strongbox && desc.attestation_security_level != SecurityLevel::StrongBox {
            return Err(InstructionExecutionError::InvariantViolation(
                "strongbox attestation required by policy".into(),
            )
            .into());
        }

        let mut lists = Vec::new();
        lists.push(&desc.software);
        lists.push(&desc.tee);
        if let Some(sb) = desc.strongbox.as_ref() {
            lists.push(sb);
        }
        if lists.iter().any(|list| list.all_applications) {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must be bound to an application ID".into(),
            )
            .into());
        }

        let attestation_app_id = desc
            .software
            .attestation_app_id
            .as_ref()
            .or(desc.tee.attestation_app_id.as_ref())
            .or(desc.strongbox.as_ref().and_then(|s| s.attestation_app_id.as_ref()))
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation missing attestationApplicationId".into(),
                )
            })?;
        if !attestation_app_id
            .package_names
            .iter()
            .any(|pkg| meta.package_names.contains(pkg))
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation package is not allowed".into(),
            )
            .into());
        }
        if !attestation_app_id
            .signature_digests
            .iter()
            .any(|digest| meta.signing_digests.contains(digest))
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation signing digest is not allowed".into(),
            )
            .into());
        }

        let purpose_ok = lists.iter().any(|list| {
            list.purposes
                .iter()
                .any(|&purpose| purpose == KM_PURPOSE_SIGN)
        });
        if !purpose_ok {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must permit SIGN purpose".into(),
            )
            .into());
        }

        let algorithm = first_present([
            desc.software.algorithm,
            desc.tee.algorithm,
            desc.strongbox.as_ref().and_then(|s| s.algorithm),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing algorithm field".into(),
            )
        })?;
        if algorithm != KM_ALGORITHM_EC {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must use EC keys".into(),
            )
            .into());
        }

        let key_size = first_present([
            desc.software.key_size_bits,
            desc.tee.key_size_bits,
            desc.strongbox.as_ref().and_then(|s| s.key_size_bits),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing keySize".into(),
            )
        })?;
        if key_size != 256 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation key must be 256 bits".into(),
            )
            .into());
        }

        let curve = first_present([
            desc.software.ec_curve,
            desc.tee.ec_curve,
            desc.strongbox.as_ref().and_then(|s| s.ec_curve),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing ecCurve".into(),
            )
        })?;
        if curve != KM_EC_CURVE_P256 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation ecCurve must be P-256".into(),
            )
            .into());
        }

        let origin = first_present([
            desc.software.origin,
            desc.tee.origin,
            desc.strongbox.as_ref().and_then(|s| s.origin),
        ])
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation missing origin field".into(),
            )
        })?;
        if origin != KM_ORIGIN_GENERATED {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must use generated keys".into(),
            )
            .into());
        }

        let has_rr = lists.iter().any(|list| list.rollback_resistance);
        if meta.require_rollback_resistance && !has_rr {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestation must be rollback-resistant".into(),
            )
            .into());
        }

        let root = desc
            .strongbox
            .as_ref()
            .and_then(|list| list.root_of_trust.clone())
            .or_else(|| desc.tee.root_of_trust.clone())
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation missing rootOfTrust".into(),
                )
            })?;
        if !root.device_locked {
            return Err(InstructionExecutionError::InvariantViolation(
                "android device must be locked".into(),
            )
            .into());
        }
        if root.verified_boot_state != KM_VERIFIED_BOOT_STATE_VERIFIED {
            return Err(InstructionExecutionError::InvariantViolation(
                "android verifiedBootState must be Verified".into(),
            )
            .into());
        }

        Ok(())
    }

    fn first_present<T: Copy>(values: [Option<T>; 3]) -> Option<T> {
        values.into_iter().flatten().next()
    }

    fn verify_marker_signature(
        proof: &AndroidMarkerKeyProof,
        challenge: &ReceiptChallenge,
    ) -> Result<(), InstructionExecutionError> {
        if let Some(signature) = &proof.marker_signature {
            let digest = Sha256::digest(challenge.iroha_hash.as_ref());
            signature
                .verify(&proof.marker_public_key, digest.as_ref())
                .map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "android marker signature does not match marker_public_key".into(),
                    )
                    .into()
        