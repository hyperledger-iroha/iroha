impl NodeHandle {
    /// Seal and store quarantined payload bytes in the local encrypted object store.
    ///
    /// The plaintext BLAKE3 digest must match the referenced quarantine record
    /// subject digest. Successful writes persist an encrypted Norito envelope
    /// and update the local object index checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine id is unknown,
    /// the payload digest does not match the quarantine record, encryption or
    /// filesystem persistence fails, or the object index lock is poisoned.
    pub fn store_moderation_quarantine_object(
        &self,
        input: ModerationQuarantineObjectInput,
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let input = normalize_moderation_quarantine_object_input(input)?;
        let quarantine = self.moderation_quarantine_record_for_object(&input.quarantine_id)?;
        let payload_digest = *blake3::hash(&input.payload).as_bytes();
        if payload_digest != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::DigestMismatch {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                expected_digest_hex: hex::encode(quarantine.subject_digest),
                actual_digest_hex: hex::encode(payload_digest),
            });
        }
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        if let Some(existing) = objects.get(&input.quarantine_id) {
            let (_, existing_envelope, _) =
                self.read_moderation_quarantine_object_envelope(root, &existing)?;
            let plaintext = open_moderation_quarantine_object(
                &existing_envelope,
                &existing,
                key_provider_binding,
                key_wrapper,
            )?;
            if existing.payload_digest != payload_digest
                || existing.captured_at_unix != input.captured_at_unix
                || existing.content_type.as_deref() != input.content_type.as_deref()
                || existing.notes.as_deref() != input.notes.as_deref()
                || plaintext.as_slice() != input.payload.as_slice()
            {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                });
            }
            return Ok(existing);
        }
        objects.ensure_insert_capacity(&input.quarantine_id)?;
        let (record, envelope_bytes) =
            seal_moderation_quarantine_object(input, key_provider_binding, key_wrapper)?;
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, &record)?;
        match fs::symlink_metadata(&envelope_path) {
            Ok(_) => {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message: err.to_string(),
                });
            }
        }
        self.finish_local_checkpoint_write(
            "moderation quarantine object envelope",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &envelope_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: err.to_string(),
        })?;
        let stored = match objects.insert(record) {
            Ok(stored) => stored,
            Err(err) => {
                if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                    let message = format!(
                        "failed to remove quarantine envelope after rejected index insertion: {cleanup}"
                    );
                    self.mark_durability_unhealthy(message.clone());
                    return Err(ModerationQuarantineObjectError::Io {
                        path: envelope_path.display().to_string(),
                        message,
                    });
                }
                return Err(err);
            }
        };
        let committed = objects.snapshot();
        if let Err(err) = self.persist_moderation_quarantine_object_index_snapshot(&committed) {
            if err.committed {
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = objects.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine object index checkpoint failure",
                    rollback,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                let message = self.record_unrecoverable_rollback(
                    "failed to remove quarantine envelope after index checkpoint failure",
                    cleanup,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message,
                });
            }
            return Err(ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(stored)
    }
    /// Read and decrypt a local quarantine payload object.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine/object record is
    /// missing, the envelope cannot be read or decoded, authentication fails,
    /// or the decrypted payload no longer matches the quarantine record digest.
    pub fn read_moderation_quarantine_object(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectPayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        let quarantine = self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload = open_moderation_quarantine_object(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
        )?;
        if *blake3::hash(payload.as_slice()).as_bytes() != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(quarantine_id),
            });
        }
        Ok(ModerationQuarantineObjectPayload {
            record,
            payload: payload.into_authorized_payload(),
        })
    }
    /// Read and authenticate an inclusive-exclusive plaintext byte range.
    ///
    /// Only ciphertext chunks intersecting `start..end` are decrypted. Each
    /// returned byte is independently authenticated against the immutable
    /// object metadata, chunk index, offset, and length.
    ///
    /// # Errors
    ///
    /// Returns an error if the range is invalid, storage or the runtime
    /// quarantine-key wrapper is unavailable or unqualified, the object is
    /// missing, or any envelope/chunk authentication check fails.
    pub fn read_moderation_quarantine_object_range(
        &self,
        quarantine_id: [u8; 16],
        start: u64,
        end: u64,
    ) -> Result<ModerationQuarantineObjectRangePayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload = open_moderation_quarantine_object_range(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
            start..end,
        )?;
        Ok(ModerationQuarantineObjectRangePayload {
            record,
            start,
            end,
            payload: payload.into_authorized_payload(),
        })
    }
    /// Rewrap one object's DEK under the wrapper's current active key.
    ///
    /// The injected wrapper must remain able to unwrap the historical key handle stored in the
    /// object envelope. Ciphertext chunks, object id, and the durable index stay byte-identical;
    /// only the context-bound wrapped DEK and its non-secret key handle are atomically replaced.
    ///
    /// # Errors
    ///
    /// Returns an error if storage or the runtime quarantine-key wrapper is unavailable or
    /// unqualified, the object is missing, old/new key operations fail, the replacement cannot be
    /// authenticated, or the atomic write fails.
    pub fn rewrap_moderation_quarantine_object_dek(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let key_provider_binding = self
            .moderation_quarantine_key_provider_binding
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnqualified)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (envelope_path, envelope, original_bytes) =
            self.read_moderation_quarantine_object_envelope(root, &record)?;
        let (replacement_record, replacement_bytes) = rewrap_moderation_quarantine_object(
            &envelope,
            &record,
            key_provider_binding,
            key_wrapper,
            key_provider_binding,
            key_wrapper,
        )?;
        if replacement_record != record {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: "DEK rewrap changed immutable object index metadata".to_owned(),
            });
        }
        if replacement_bytes == original_bytes {
            return Ok(record);
        }
        let replacement_envelope = decode_moderation_quarantine_object_envelope(
            &replacement_bytes,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|error| ModerationQuarantineObjectError::Codec {
            message: error.to_string(),
        })?;
        open_moderation_quarantine_object(
            &replacement_envelope,
            &replacement_record,
            key_provider_binding,
            key_wrapper,
        )?;
        self.finish_local_checkpoint_write(
            "moderation quarantine object DEK rewrap",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &replacement_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok(record)
    }
}
