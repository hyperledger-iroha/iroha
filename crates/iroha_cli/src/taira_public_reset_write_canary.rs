//! Shared exact WriteCanary driver for public SSH and native private staging.

use super::*;

/// Transport-specific custody surrounding one shared durable write-child protocol.
pub(in crate::taira_public_reset) trait CoreWriteTransport {
    fn core_write_identity(&self, phase: &str) -> Result<CoreWriteIdentity>;
    fn core_receipt_root(&self) -> &Path;
    fn core_client_args(&self) -> Result<(Vec<OsString>, Vec<File>)>;
    fn core_onboarding_token(&self) -> Result<File>;
    fn core_run_process(
        &mut self,
        args: Vec<OsString>,
        files: Vec<File>,
        deadline: Instant,
        recovery_only: bool,
    ) -> Result<ProcessOutput>;
    fn core_publish_receipt(&self, name: &str, value: &norito::json::Value) -> Result<()>;
    fn coordinate_shared_prepared_mutation(
        &mut self,
        operation: &str,
        kind: &str,
        phase: &str,
        key: &str,
        prepared: Option<&[u8]>,
        digest: &str,
        transaction_hash: &str,
        evidence: Option<&[u8]>,
        recovery_only: bool,
        timeout_secs: u64,
    ) -> Result<RetainedPreparedMutation>;

    fn run_local_cli_until(
        &mut self,
        args: Vec<OsString>,
        files: Vec<File>,
        _timeout_secs: u64,
        deadline: Instant,
        recovery_only: bool,
        label: &str,
    ) -> Result<Vec<u8>> {
        require_success(
            self.core_run_process(args, files, deadline, recovery_only)?,
            label,
        )
    }

    fn run_core_write_child(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        index: usize,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("journaled prepared child deadline overflow"))?;
        let prepared =
            self.prepare_write_canary_child_until(deadline, timeout_secs, phase, kind)?;
        if !matches!(
            prepared.state.as_str(),
            "prepared" | "submitted" | "applied"
        ) {
            return Err(eyre!("prepared write store returned an unsupported state"));
        }
        run_journaled_submitted_mutation(progress, index, || {
            let already_applied = prepared.state == "applied";
            let already_submitted = prepared.state == "submitted";
            let proof_required = prepared.requires_onboarding_proof(kind)?;
            let prepared = if already_applied || already_submitted || proof_required {
                prepared
            } else {
                let identity = self.core_write_identity(phase)?;
                self.coordinate_shared_prepared_mutation(
                    "submitted",
                    kind,
                    phase,
                    &child_mutation_idempotency_key(&identity.authorization_nonce, phase, kind),
                    None,
                    "",
                    "",
                    None,
                    false,
                    remaining_seconds(deadline)?,
                )?
            };
            let outcome = self.run_write_canary_prepared_until(
                deadline,
                phase,
                kind,
                &prepared,
                already_applied || already_submitted || proof_required,
            )?;
            if let PreparedMutationOutcome::Applied { value, evidence } = &outcome {
                if !already_applied {
                    self.mark_shared_prepared_applied(phase, kind, &prepared, evidence, deadline)?;
                }
                self.core_publish_receipt(
                    &format!("{}-{phase}.json", kind.replace('_', "-")),
                    value,
                )?;
            }
            Ok(outcome)
        })
    }

    fn recover_core_write_child(
        &mut self,
        deadline: Instant,
        phase: &str,
        kind: &str,
    ) -> Result<PreparedMutationOutcome> {
        let prepared =
            self.fetch_shared_prepared_mutation(phase, kind, true, remaining_seconds(deadline)?)?;
        if !prepared.write_recovery_requires_observation(kind)? {
            return Ok(PreparedMutationOutcome::Rejected(
                "prepared_child_not_submitted".to_owned(),
            ));
        }
        let outcome =
            self.run_write_canary_prepared_until(deadline, phase, kind, &prepared, true)?;
        if let PreparedMutationOutcome::Applied { evidence, .. } = &outcome {
            if prepared.state != "applied" {
                self.mark_shared_prepared_applied(phase, kind, &prepared, evidence, deadline)?;
            }
        }
        Ok(outcome)
    }

    fn prepare_write_canary_child_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<RetainedPreparedMutation> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.core_write_identity(phase)?.authorization_nonce,
            phase,
            kind,
        );
        let existing = self.coordinate_shared_prepared_mutation(
            "fetch",
            kind,
            phase,
            &idempotency_key,
            None,
            "",
            "",
            None,
            false,
            remaining_seconds(deadline)?,
        )?;
        if existing.state != "absent" {
            return Ok(existing);
        }
        let prerequisite = mutation_predecessor_kind(kind, phase)
            .map(|predecessor| {
                self.fetch_shared_prepared_mutation(
                    phase,
                    predecessor,
                    false,
                    remaining_seconds(deadline)?,
                )
            })
            .transpose()?
            .filter(|value| value.state == "applied");
        if mutation_predecessor_kind(kind, phase).is_some() && prerequisite.is_none() {
            return Err(eyre!(
                "write child preparation requires its exact Applied predecessor envelope"
            ));
        }
        let (candidate, _prepare_report) = self.run_write_canary_prepare_until(
            deadline,
            timeout_secs,
            phase,
            kind,
            &idempotency_key,
            prerequisite.as_ref(),
        )?;
        let candidate_sha256 = sha256_hex(&candidate);
        let transaction_hash = prepared_envelope_transaction_hash(&candidate)?;
        self.coordinate_shared_prepared_mutation(
            "prepare",
            kind,
            phase,
            &idempotency_key,
            Some(&candidate),
            &candidate_sha256,
            &transaction_hash,
            None,
            false,
            remaining_seconds(deadline)?,
        )
    }

    fn fetch_shared_prepared_mutation(
        &mut self,
        phase: &str,
        kind: &str,
        recovery_only: bool,
        timeout_secs: u64,
    ) -> Result<RetainedPreparedMutation> {
        self.coordinate_shared_prepared_mutation(
            "fetch",
            kind,
            phase,
            &child_mutation_idempotency_key(
                &self.core_write_identity(phase)?.authorization_nonce,
                phase,
                kind,
            ),
            None,
            "",
            "",
            None,
            recovery_only,
            timeout_secs,
        )
    }

    fn mark_shared_prepared_applied(
        &mut self,
        phase: &str,
        kind: &str,
        prepared: &RetainedPreparedMutation,
        evidence: &[u8],
        deadline: Instant,
    ) -> Result<()> {
        let result = self.coordinate_shared_prepared_mutation(
            "applied",
            kind,
            phase,
            &child_mutation_idempotency_key(
                &self.core_write_identity(phase)?.authorization_nonce,
                phase,
                kind,
            ),
            None,
            "",
            "",
            Some(evidence),
            true,
            remaining_seconds(deadline)?,
        )?;
        if result.state != "applied"
            || result.sha256 != prepared.sha256
            || result.transaction_hash != prepared.transaction_hash
        {
            return Err(eyre!(
                "shared prepared mutation Applied marker changed its immutable envelope"
            ));
        }
        Ok(())
    }

    fn run_write_canary_prepare_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        prerequisite: Option<&RetainedPreparedMutation>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let scratch = self
            .core_receipt_root()
            .join(format!(".{idempotency_key}.candidate.next"));
        if scratch.exists() {
            let metadata = fs::symlink_metadata(&scratch)?;
            #[cfg(unix)]
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.nlink() != 1
            {
                return Err(eyre!("prepared envelope scratch file has unsafe custody"));
            }
            fs::remove_file(&scratch)?;
            sync_directory(&self.core_receipt_root())?;
        }
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&scratch)?;
        let mut output_reader = output_file.try_clone()?;
        let output_path = inherited_file_path(&output_file)?;
        let (mut args, mut inherited_files) = self.write_canary_base_args(
            phase,
            kind,
            idempotency_key,
            timeout_secs,
            WriteCanaryChildAction::Prepare,
        )?;
        WriteCanaryChildAction::Prepare.append_envelope_args(&mut args, output_file.as_raw_fd());
        debug_assert_eq!(
            output_path,
            PathBuf::from(format!("/proc/self/fd/{}", output_file.as_raw_fd()))
        );
        inherited_files.push(output_file);
        if let Some(prerequisite) = prerequisite {
            let file = self.open_retained_prepared_envelope(prerequisite)?;
            args.push(OsString::from("--prerequisite-envelope-fd"));
            args.push(file.as_raw_fd().to_string().into());
            inherited_files.push(file);
        }
        let stdout = self.run_local_cli_until(
            args,
            inherited_files,
            timeout_secs,
            deadline,
            false,
            "prepare exact write-canary child envelope",
        )?;
        let report = parse_json_report(&stdout, "prepared write child")?;
        let outcome = report
            .as_object()
            .and_then(|object| object.get("recovery_outcome"))
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write child report omits recovery_outcome"))?;
        if !matches!(outcome, "Prepared" | "ProofRequired") {
            return Err(eyre!(
                "prepared write child produced an invalid preparation outcome"
            ));
        }
        validate_core_write_report(
            &report,
            &self.core_write_identity(phase)?,
            phase,
            kind,
            idempotency_key,
            outcome,
            None,
        )?;
        output_reader.rewind()?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut output_reader)
            .take(u64::try_from(MAX_PREPARED_ENVELOPE_BYTES + 1).expect("bounded"))
            .read_to_end(&mut bytes)?;
        output_reader.sync_all()?;
        fs::remove_file(&scratch)?;
        sync_directory(&self.core_receipt_root())?;
        if bytes.is_empty() || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES {
            return Err(eyre!("prepared write child envelope is empty or oversized"));
        }
        validate_prepared_report_envelope_bytes(&report, &bytes)?;
        let transaction_hash = prepared_envelope_transaction_hash(&bytes)?;
        if (outcome == "ProofRequired") != transaction_hash.is_empty() {
            return Err(eyre!(
                "prepared write child proof requirement does not match its tagged envelope"
            ));
        }
        Ok((bytes, stdout))
    }

    fn run_write_canary_prepared_until(
        &mut self,
        deadline: Instant,
        phase: &str,
        kind: &str,
        prepared: &RetainedPreparedMutation,
        recover_only: bool,
    ) -> Result<PreparedMutationOutcome> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.core_write_identity(phase)?.authorization_nonce,
            phase,
            kind,
        );
        let file = self.open_retained_prepared_envelope(prepared)?;
        let action = if recover_only {
            WriteCanaryChildAction::Recover
        } else {
            WriteCanaryChildAction::Submit
        };
        let (mut args, mut inherited_files) = self.write_canary_base_args(
            phase,
            kind,
            &idempotency_key,
            remaining_seconds(deadline)?,
            action,
        )?;
        action.append_envelope_args(&mut args, file.as_raw_fd());
        inherited_files.push(file);
        let process = self.core_run_process(args, inherited_files, deadline, recover_only)?;
        let value = parse_prepared_child_report(process, "exact prepared write child")?;
        let outcome = value
            .as_object()
            .and_then(|object| object.get("recovery_outcome"))
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write child report omits recovery_outcome"))?;
        validate_core_write_report(
            &value,
            &self.core_write_identity(phase)?,
            phase,
            kind,
            &idempotency_key,
            outcome,
            Some(prepared),
        )?;
        core_prepared_write_outcome(value, prepared)
    }

    fn write_canary_base_args(
        &self,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        timeout_secs: u64,
        action: WriteCanaryChildAction,
    ) -> Result<(Vec<OsString>, Vec<File>)> {
        let identity = self.core_write_identity(phase)?;
        let (mut args, mut inherited_files) = self.core_client_args()?;
        args.extend([
            OsString::from("taira"),
            OsString::from("write-canary"),
            OsString::from("--public-root"),
            OsString::from(&identity.origin),
            OsString::from("--timeout-secs"),
            OsString::from(timeout_secs.to_string()),
            OsString::from("--operation"),
            OsString::from(match kind {
                "onboarding" => "onboarding",
                "faucet" => "faucet",
                "write_canary" => "final-canary",
                _ => return Err(eyre!("unsupported prepared write child kind")),
            }),
            OsString::from("--authorization-sha256"),
            OsString::from(&identity.authorization_sha256),
            OsString::from("--authorization-nonce"),
            OsString::from(&identity.authorization_nonce),
            OsString::from("--mutation-phase"),
            OsString::from(phase),
            OsString::from("--idempotency-key"),
            OsString::from(idempotency_key),
            OsString::from("--execution-expires-at-unix-ms"),
            OsString::from(identity.execution_expires_at_unix_ms.to_string()),
        ]);
        if kind == "faucet" {
            args.extend([
                OsString::from("--faucet-authority"),
                OsString::from(identity.faucet_policy.faucet_authority().to_string()),
                OsString::from("--faucet-asset-id"),
                OsString::from(identity.faucet_policy.asset_definition_id().to_string()),
                OsString::from("--faucet-amount"),
                OsString::from(identity.faucet_policy.amount().to_string()),
            ]);
        }
        if kind == "write_canary" && action == WriteCanaryChildAction::Prepare {
            args.extend([
                OsString::from("--predecessor-faucet-authority"),
                OsString::from(identity.faucet_policy.faucet_authority().to_string()),
                OsString::from("--predecessor-faucet-asset-id"),
                OsString::from(identity.faucet_policy.asset_definition_id().to_string()),
                OsString::from("--predecessor-faucet-amount"),
                OsString::from(identity.faucet_policy.amount().to_string()),
            ]);
        }
        if kind == "onboarding" && action != WriteCanaryChildAction::Recover {
            let token_file = self.core_onboarding_token()?;
            args.push(OsString::from("--onboarding-token-fd"));
            args.push(token_file.as_raw_fd().to_string().into());
            inherited_files.push(token_file);
        }
        args.push(OsString::from("--json"));
        Ok((args, inherited_files))
    }

    fn open_retained_prepared_envelope(&self, prepared: &RetainedPreparedMutation) -> Result<File> {
        if prepared.bytes.is_empty()
            || prepared.bytes.len() > MAX_PREPARED_ENVELOPE_BYTES
            || sha256_hex(&prepared.bytes) != prepared.sha256
        {
            return Err(eyre!("retained prepared envelope identity is invalid"));
        }
        let root = self.core_receipt_root().join("prepared-envelopes-v1");
        ensure_private_directory(&root)?;
        let name = format!("{}.json", prepared.sha256);
        publish_private_noreplace(&root, &name, &prepared.bytes)?;
        let path = root.join(name);
        let file = File::open(&path)?;
        let metadata = file.metadata()?;
        #[cfg(unix)]
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(eyre!("retained prepared envelope has unsafe custody"));
        }
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    struct Progress(Rc<RefCell<Vec<&'static str>>>);
    impl RecoveryProgress for Progress {
        fn mark_submitted(&mut self, _index: usize) -> Result<()> {
            self.0.borrow_mut().push("durable_submitted");
            Ok(())
        }
        fn mark_applied(&mut self, _index: usize) -> Result<()> {
            self.0.borrow_mut().push("durable_applied");
            Ok(())
        }
    }

    struct InterruptedTransport {
        state: &'static str,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    fn fixture_identity() -> CoreWriteIdentity {
        let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
        let key =
            iroha_crypto::KeyPair::try_from_seed(vec![0x41; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("fixture key");
        let account = AccountId::new(key.public_key().clone());
        CoreWriteIdentity {
            authorization_sha256: "a1".repeat(32),
            authorization_nonce: "a".repeat(32),
            not_before_unix_ms: 1,
            execution_expires_at_unix_ms: u64::MAX,
            origin: "http://127.0.0.1:28080".into(),
            chain_id: super::super::super::CHAIN_ID.into(),
            genesis_hash: "b2".repeat(32),
            onboarding_request: AccountOnboardingPlanRequestV1::try_new(
                crate::taira::canary_alias(key.public_key()),
                &account,
                std::iter::empty(),
            )
            .expect("fixture request"),
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            faucet_policy: AccountFaucetPolicyV1::try_new(
                account,
                crate::taira::DEFAULT_GAS_ASSET_ID.parse().expect("asset"),
                "1".parse().expect("amount"),
            )
            .expect("fixture faucet"),
        }
    }

    impl InterruptedTransport {
        fn retained(&self) -> RetainedPreparedMutation {
            RetainedPreparedMutation {
                state: self.state.into(),
                bytes: vec![1],
                sha256: "c3".repeat(32),
                transaction_hash: "d4".repeat(32),
            }
        }
    }

    impl CoreWriteTransport for InterruptedTransport {
        fn core_write_identity(&self, _phase: &str) -> Result<CoreWriteIdentity> {
            Ok(fixture_identity())
        }
        fn core_receipt_root(&self) -> &Path {
            unreachable!("mock child never opens files")
        }
        fn core_client_args(&self) -> Result<(Vec<OsString>, Vec<File>)> {
            unreachable!("mock child never loads signers")
        }
        fn core_onboarding_token(&self) -> Result<File> {
            unreachable!("mock child never opens tokens")
        }
        fn core_run_process(
            &mut self,
            _args: Vec<OsString>,
            _files: Vec<File>,
            _deadline: Instant,
            _recovery_only: bool,
        ) -> Result<ProcessOutput> {
            unreachable!("mock models process boundary directly")
        }
        fn core_publish_receipt(&self, _name: &str, _value: &norito::json::Value) -> Result<()> {
            unreachable!("Pending has no receipt")
        }
        fn coordinate_shared_prepared_mutation(
            &mut self,
            operation: &str,
            _kind: &str,
            _phase: &str,
            _key: &str,
            _prepared: Option<&[u8]>,
            _digest: &str,
            _transaction_hash: &str,
            _evidence: Option<&[u8]>,
            _recovery_only: bool,
            _timeout_secs: u64,
        ) -> Result<RetainedPreparedMutation> {
            assert_eq!(
                operation, "submitted",
                "retained intent must never be prepared again"
            );
            self.events.borrow_mut().push("store_submitted");
            self.state = "submitted";
            Ok(self.retained())
        }
        fn prepare_write_canary_child_until(
            &mut self,
            _deadline: Instant,
            _timeout_secs: u64,
            _phase: &str,
            _kind: &str,
        ) -> Result<RetainedPreparedMutation> {
            self.events.borrow_mut().push("fetch_retained");
            Ok(self.retained())
        }
        fn run_write_canary_prepared_until(
            &mut self,
            _deadline: Instant,
            _phase: &str,
            _kind: &str,
            _prepared: &RetainedPreparedMutation,
            recover_only: bool,
        ) -> Result<PreparedMutationOutcome> {
            self.events
                .borrow_mut()
                .push(if recover_only { "recover" } else { "submit" });
            Ok(PreparedMutationOutcome::Pending)
        }
    }

    #[test]
    fn retained_submitted_driver_entries_only_observe_the_same_envelope() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut transport = InterruptedTransport {
            state: "submitted",
            events: events.clone(),
        };
        let mut progress = Progress(events.clone());
        for _ in 0..2 {
            let error = transport
                .run_core_write_child(&mut progress, 0, 30, "pre_edge", "onboarding")
                .expect_err("Pending must retain durable intent for another read-only recovery");
            assert!(
                error.root_cause().is::<LocalMutationRecoveryPending>(),
                "mock child must reach its Pending outcome: {error:#}"
            );
        }
        assert_eq!(
            *events.borrow(),
            [
                "fetch_retained",
                "durable_submitted",
                "recover",
                "fetch_retained",
                "durable_submitted",
                "recover"
            ]
        );
    }

    #[test]
    fn fresh_prepared_dispatch_has_durable_intent_before_the_child_process() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut transport = InterruptedTransport {
            state: "prepared",
            events: events.clone(),
        };
        let mut progress = Progress(events.clone());
        let error = transport
            .run_core_write_child(&mut progress, 0, 30, "pre_edge", "onboarding")
            .expect_err("interruption is Pending");
        assert!(
            error.root_cause().is::<LocalMutationRecoveryPending>(),
            "mock child must reach its Pending outcome: {error:#}"
        );
        assert_eq!(
            *events.borrow(),
            [
                "fetch_retained",
                "durable_submitted",
                "store_submitted",
                "submit"
            ]
        );
        events.borrow_mut().clear();
        let error = transport
            .run_core_write_child(&mut progress, 0, 30, "pre_edge", "onboarding")
            .expect_err("retry remains observation-only");
        assert!(
            error.root_cause().is::<LocalMutationRecoveryPending>(),
            "mock recovery must reach its Pending outcome: {error:#}"
        );
        assert_eq!(
            *events.borrow(),
            ["fetch_retained", "durable_submitted", "recover"]
        );
    }
}
