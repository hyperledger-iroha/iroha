//! Retain native canary diagnostics before requiring an exact Applied proof.
use super::*;

impl Canary<'_> {
    pub(super) fn retain_receipt(
        &self,
        operation: &str,
        kind: &str,
        prepare: bool,
        bytes: &[u8],
    ) -> Result<Option<u64>> {
        let phase = if prepare { "prepare" } else { "submit" };
        let path = self.directory.join(format!("{operation}-{phase}.json"));
        // An unsuccessful or malformed public response is still evidence. Sync
        // both the bytes and directory before any parsing or proof assertion.
        private_file(&path, bytes)?;
        File::open(self.directory)?.sync_all()?;
        let receipt: Value = json::from_slice(bytes)?;
        ensure!(
            text(&receipt, "status")? == "ok",
            "native prepared canary did not succeed; retained receipt: {}",
            path.display()
        );
        if prepare {
            return Ok(None);
        }
        let retained = fs::read(self.directory.join(format!("{operation}.prepared.json")))?;
        let evidence = text(&receipt, "evidence")?;
        ensure!(
            text(&receipt, "prepared_envelope_sha256")? == hex(&iroha_crypto::sha256(&retained))
                && field(&receipt, "prepared_envelope_size")?.as_u64()
                    == Some(retained.len() as u64)
                && text(&receipt, "authorization_sha256")? == self.authorization
                && text(&receipt, "authorization_nonce")? == self.nonce
                && text(&receipt, "mutation_kind")? == kind
                && text(&receipt, "mutation_phase")? == "pre_edge"
                && text(&receipt, "idempotency_key")? == idempotency(&self.nonce, kind)
                && !evidence.is_empty(),
            "native canary proof receipt does not bind the exact retained operation; retained receipt: {}",
            path.display()
        );
        // Status=ok means the child classified the exact operation. Only its
        // state-resolved Applied proof may drive the DKG height descriptor.
        let outcome = text(&receipt, "recovery_outcome")?;
        ensure!(
            outcome == "Applied",
            "native {operation} was not Applied: recovery_outcome={outcome:?}, evidence={evidence:?}; retained receipt: {}",
            path.display()
        );
        let height = field(&receipt, "applied_block_height")?
            .as_u64()
            .filter(|height| *height > 1)
            .ok_or_else(|| eyre!("native canary omitted its proved Applied height"))?;
        Ok(Some(height))
    }
}

fn fixture(directory: &Path) -> Canary<'_> {
    Canary {
        binary: Path::new("unused"),
        directory,
        config: Path::new("unused"),
        root: "http://127.0.0.1:8080".to_owned(),
        nonce: "a".repeat(32),
        authorization: "b".repeat(64),
        expires_ms: 1,
        faucet: std::array::from_fn(|_| String::new()),
    }
}

fn receipt(canary: &Canary<'_>, envelope: &[u8]) -> Value {
    norito::json!({
        "status": "ok",
        "prepared_envelope_sha256": (hex(&iroha_crypto::sha256(envelope))),
        "prepared_envelope_size": (envelope.len() as u64),
        "authorization_sha256": (canary.authorization.clone()),
        "authorization_nonce": (canary.nonce.clone()),
        "mutation_kind": "write_canary",
        "mutation_phase": "pre_edge",
        "idempotency_key": (idempotency(&canary.nonce, "write_canary")),
        "evidence": "exact-transaction-proof",
        "recovery_outcome": "Applied",
        "applied_block_height": 4
    })
}

#[test]
fn failed_canary_receipts_are_retained_before_parse_and_outcome_checks() -> Result<()> {
    for outcome in ["Pending", "Rejected"] {
        let directory = tempfile::tempdir()?;
        let canary = fixture(directory.path());
        let envelope = b"retained exact operation";
        private_file(
            &directory.path().join("final-canary.prepared.json"),
            envelope,
        )?;
        let mut value = receipt(&canary, envelope);
        let object = value.as_object_mut().unwrap();
        object.insert("recovery_outcome".to_owned(), Value::from(outcome));
        object.insert("applied_block_height".to_owned(), Value::Null);
        object.insert(
            "evidence".to_owned(),
            Value::from("observation_unavailable"),
        );
        let bytes = json::to_vec(&value)?;
        let error = canary
            .retain_receipt("final-canary", "write_canary", false, &bytes)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("recovery_outcome={outcome:?}")));
        assert!(error.contains("observation_unavailable"));
        assert!(!error.contains("does not bind"));
        assert!(!error.contains(&canary.authorization));
        assert!(!error.contains(&canary.nonce));
        let path = directory.path().join("final-canary-submit.json");
        assert_eq!(fs::read(&path)?, bytes);
        assert_eq!(fs::metadata(&path)?.mode() & 0o077, 0);
        // Failure evidence is exclusive custody, never overwritten on a retry.
        assert!(
            canary
                .retain_receipt("final-canary", "write_canary", false, b"replacement")
                .is_err()
        );
        assert_eq!(fs::read(&path)?, bytes);
    }
    let directory = tempfile::tempdir()?;
    let canary = fixture(directory.path());
    assert_eq!(
        canary.retain_receipt("prepared", "write_canary", true, br#"{"status":"ok"}"#)?,
        None
    );
    assert_eq!(
        fs::read(directory.path().join("prepared-prepare.json"))?,
        br#"{"status":"ok"}"#
    );
    for (operation, bytes) in [
        ("malformed", b"{".as_slice()),
        ("unsuccessful", br#"{"status":"error"}"#.as_slice()),
    ] {
        assert!(
            canary
                .retain_receipt(operation, operation, true, bytes)
                .is_err()
        );
        assert_eq!(
            fs::read(directory.path().join(format!("{operation}-prepare.json")))?,
            bytes
        );
    }
    Ok(())
}

#[test]
fn retained_canary_receipt_requires_every_binding_and_applied_height() -> Result<()> {
    let changes = [
        ("prepared_envelope_sha256", Value::from("foreign")),
        ("prepared_envelope_size", Value::from(0_u64)),
        ("authorization_sha256", Value::from("foreign")),
        ("authorization_nonce", Value::from("foreign")),
        ("mutation_kind", Value::from("faucet")),
        ("mutation_phase", Value::from("post_edge")),
        ("idempotency_key", Value::from("foreign")),
        ("evidence", Value::from("")),
        ("applied_block_height", Value::from(1_u64)),
        ("applied_block_height", Value::Null),
    ];
    for change in std::iter::once(None).chain(changes.into_iter().map(Some)) {
        let directory = tempfile::tempdir()?;
        let canary = fixture(directory.path());
        let envelope = b"retained exact operation";
        private_file(
            &directory.path().join("final-canary.prepared.json"),
            envelope,
        )?;
        let mut value = receipt(&canary, envelope);
        if let Some((field, invalid)) = &change {
            value
                .as_object_mut()
                .unwrap()
                .insert((*field).to_owned(), invalid.clone());
        }
        let bytes = json::to_vec(&value)?;
        let result = canary.retain_receipt("final-canary", "write_canary", false, &bytes);
        if change.is_some() {
            assert!(result.is_err(), "accepted invalid receipt {change:?}");
        } else {
            assert_eq!(result?, Some(4));
        }
        assert_eq!(
            fs::read(directory.path().join("final-canary-submit.json"))?,
            bytes
        );
    }
    Ok(())
}
