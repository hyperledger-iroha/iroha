// Included in executor_model so abandonment shares its durable journal and rollback machinery.

const ABANDONMENT_SCHEMA_V1: &str = "iroha.taira.public-reset.abandonment.v1";
const ABANDONMENT_REASON: &str =
    "operator explicitly abandoned the unproven testnet; transaction outcomes remain unresolved";

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AbandonmentReceiptV1 {
    schema: String,
    action: String,
    transaction_outcome: String,
    journal_sha256: String,
    // Preserve the exact original bytes, including whitespace and every mutation state.
    journal_json: String,
}

fn validate_abandonment_source(state: &JournalV1) -> Result<()> {
    validate_resumable_journal(state, state)?;
    if state.status != "recovery_pending"
        || state.phase != ExecutionStep::Canary.label()
        || EXECUTION_STEPS.get(usize::from(state.next_step)) != Some(&ExecutionStep::Canary)
        || state
            .touched_validators
            .iter()
            .map(String::as_str)
            .ne(VALIDATOR_SLUGS)
        || state.edge_touched
        || state.edge_rollback_complete
        || state.rollback_next_validator != 0
        || !state.rollback_failures.is_empty()
        || !state
            .recovery_intent
            .as_ref()
            .is_some_and(|intent| usize::from(intent.next_mutation) < intent.mutations.len())
    {
        return Err(eyre!(
            "abandonment requires an unresolved canary with all four validators and an untouched edge"
        ));
    }
    Ok(())
}

impl DurableJournal {
    fn preserve_abandonment(&self, expected_sha256: &str) -> Result<PathBuf> {
        validate_lower_hex("expected journal SHA-256", expected_sha256, 64)?;
        for terminal in [
            &self.receipt_path,
            &self.deployment_receipt_path,
            &self.aborted_receipt_path,
            &self.rollback_receipt_path,
        ] {
            match fs::symlink_metadata(terminal) {
                Ok(_) => return Err(eyre!("proven or terminal deployments cannot be abandoned")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let (current, current_bytes) =
            read_private_json::<JournalV1>(&self.current_path, "abandonment journal")?;
        if current != self.state {
            return Err(eyre!("abandonment journal changed while its lock was held"));
        }
        let directory = self.directory.join("abandoned");
        let receipt_path = directory.join(format!("{}.json", self.state.authorization_sha256));
        let receipt = if directory.try_exists()? {
            validate_owner_private_dir(&directory, "abandonment receipt directory")?;
            if receipt_path.try_exists()? {
                Some(
                    read_private_json::<AbandonmentReceiptV1>(
                        &receipt_path,
                        "abandonment receipt",
                    )?
                    .0,
                )
            } else {
                None
            }
        } else {
            None
        };
        let receipt = match receipt {
            Some(receipt) => receipt,
            None => {
                validate_abandonment_source(&current)?;
                if sha256_hex(&current_bytes) != expected_sha256 {
                    return Err(eyre!(
                        "abandonment journal SHA-256 differs from the explicit expected digest"
                    ));
                }
                AbandonmentReceiptV1 {
                    schema: ABANDONMENT_SCHEMA_V1.to_owned(),
                    action: "abandon_unproven_testnet_and_rollback".to_owned(),
                    transaction_outcome: "unresolved".to_owned(),
                    journal_sha256: expected_sha256.to_owned(),
                    journal_json: String::from_utf8(current_bytes.clone())?,
                }
            }
        };
        if receipt.schema != ABANDONMENT_SCHEMA_V1
            || receipt.action != "abandon_unproven_testnet_and_rollback"
            || receipt.transaction_outcome != "unresolved"
            || receipt.journal_sha256 != expected_sha256
            || sha256_hex(receipt.journal_json.as_bytes()) != expected_sha256
        {
            return Err(eyre!(
                "abandonment receipt does not preserve the exact unresolved journal"
            ));
        }
        let original: JournalV1 = json::from_str(&receipt.journal_json)?;
        validate_abandonment_source(&original)?;
        validate_resumable_journal(&current, &original)?;
        match current.status.as_str() {
            "recovery_pending" if receipt.journal_json.as_bytes() == current_bytes => {}
            "rolling_back" => {
                let mut expected = original;
                expected.status = "rolling_back".to_owned();
                expected.phase = "rollback".to_owned();
                expected.recovery_intent = None;
                expected.failure_summary =
                    stable_error_record("rollback", "forward", &eyre!(ABANDONMENT_REASON));
                expected.rollback_next_validator = current.rollback_next_validator;
                expected
                    .rollback_failures
                    .clone_from(&current.rollback_failures);
                if expected != current {
                    return Err(eyre!(
                        "rollback journal is not the preserved abandonment successor"
                    ));
                }
            }
            _ => {
                return Err(eyre!(
                    "current journal is not the exact abandonment source or rollback successor"
                ));
            }
        }
        let encoded = json::to_json(&receipt)?;
        if encoded.len() as u64 >= MAX_JSON_BYTES {
            return Err(eyre!(
                "abandonment evidence exceeds the durable receipt size bound"
            ));
        }
        if !directory.try_exists()? {
            fs::create_dir(&directory)?;
            set_owner_only_dir(&directory)?;
            sync_directory(&self.directory)?;
        }
        validate_owner_private_dir(&directory, "abandonment receipt directory")?;
        // Publish and fsync before clearing recovery_intent or issuing any host mutation.
        publish_json_no_replace(&receipt_path, &receipt)?;
        Ok(receipt_path)
    }
}

/// Preserve an explicitly selected unresolved testnet attempt before its existing full rollback.
pub(super) fn abandon_pending_attempt<T: ResetTransport>(
    inventory: &InventoryV1,
    transport: &mut T,
    journal: &mut DurableJournal,
    expected_sha256: &str,
) -> Result<PathBuf> {
    let receipt_path = journal.preserve_abandonment(expected_sha256)?;
    if journal.state.status == "recovery_pending" {
        begin_rollback_after_preparation_failure(journal, &eyre!(ABANDONMENT_REASON))?;
    }
    resume_rollback(inventory, transport, journal)?;
    Ok(receipt_path)
}
