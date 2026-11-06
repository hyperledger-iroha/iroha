//! Explicit Ironwood V3 wallet operations. These functions grant no ledger/finality authority.
use super::*;
use iroha_data_model::privacy::{OrchardHalo2ActionsStatementV1, PrivacyValueBalanceDirectionV1};
use orchard::keys::IncomingViewingKey;

/// Derive a public recipient address. Spending material never appears in the result.
/// # Errors
/// Rejects a noncanonical spending key.
pub fn recipient_address_v1(
    key: [u8; 32],
    internal: bool,
    index: u32,
) -> Result<[u8; 43], OrchardSpendInputErrorV1> {
    let key = Option::<SpendingKey>::from(SpendingKey::from_bytes(key))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    Ok(FullViewingKey::from(&key)
        .address_at(
            index,
            if internal {
                Scope::Internal
            } else {
                Scope::External
            },
        )
        .to_raw_address_bytes())
}

/// Parse an ordinary recipient or separately derived payer-change output, without a payee key.
/// # Errors
/// Rejects a noncanonical recipient address.
pub fn output_from_wallet_parts_v1(
    recipient: [u8; 43],
    value: u64,
    memo: &[u8; 512],
) -> Result<IronwoodOutputProverInputV1, OrchardSpendInputErrorV1> {
    let recipient = Option::<Address>::from(Address::from_raw_address_bytes(&recipient))
        .ok_or(OrchardSpendInputErrorV1::Recipient)?;
    Ok(IronwoodOutputProverInputV1::new(
        None, recipient, value, memo,
    ))
}

/// Parse a V3 note opening and authenticate its complete path to the independently selected anchor.
/// # Errors
/// Rejects invalid keys, ownership, note components, path fields or an anchor mismatch.
pub fn spend_from_wallet_parts_v1(
    spending_key: zeroize::Zeroizing<[u8; 32]>,
    recipient: [u8; 43],
    value: u64,
    rho: [u8; 32],
    random_seed: [u8; 32],
    merkle_path: (u32, &[[u8; 32]; ORCHARD_TREE_DEPTH_V1 as usize]),
    expected_anchor: [u8; 32],
) -> Result<OrchardSpendProverInputV1, OrchardSpendInputErrorV1> {
    let (leaf_position, authentication_path) = merkle_path;
    let spending_key = Option::<SpendingKey>::from(SpendingKey::from_bytes(*spending_key))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    let recipient = Option::<Address>::from(Address::from_raw_address_bytes(&recipient))
        .ok_or(OrchardSpendInputErrorV1::Recipient)?;
    if FullViewingKey::from(&spending_key)
        .scope_for_address(&recipient)
        .is_none()
    {
        return Err(OrchardSpendInputErrorV1::RecipientOwnership);
    }
    let rho = Option::<Rho>::from(Rho::from_bytes(&rho)).ok_or(OrchardSpendInputErrorV1::Rho)?;
    let random_seed = Option::<RandomSeed>::from(RandomSeed::from_bytes(random_seed, &rho))
        .ok_or(OrchardSpendInputErrorV1::RandomSeed)?;
    let note = Option::<Note>::from(Note::from_parts(
        recipient,
        NoteValue::from_raw(value),
        rho,
        random_seed,
        NoteVersion::V3,
    ))
    .ok_or(OrchardSpendInputErrorV1::Note)?;
    let path = authentication_path
        .iter()
        .enumerate()
        .map(|(index, bytes)| {
            Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(bytes))
                .ok_or(OrchardSpendInputErrorV1::AuthenticationPath { index })
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| OrchardSpendInputErrorV1::AuthenticationPathLength)?;
    let merkle_path = MerklePath::from_parts(leaf_position, path);
    let anchor = merkle_path
        .root(ExtractedNoteCommitment::from(note.commitment()))
        .to_bytes();
    if anchor != expected_anchor {
        return Err(OrchardSpendInputErrorV1::AnchorMismatch);
    }
    Ok(OrchardSpendProverInputV1::new(
        spending_key,
        note,
        merkle_path,
    ))
}

/// Decrypted note retained inside the native wallet. Debug output deliberately omits note material.
pub struct DecryptedIronwoodNoteV1 {
    action_index: usize,
    note: Note,
    memo: [u8; 512],
    nullifier: Option<[u8; 32]>,
}
impl core::fmt::Debug for DecryptedIronwoodNoteV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DecryptedIronwoodNoteV1(<restricted>)")
    }
}
impl DecryptedIronwoodNoteV1 {
    /// Original action position in the bundle.
    pub fn action_index(&self) -> usize {
        self.action_index
    }
    /// Exact decrypted destination.
    pub fn recipient(&self) -> [u8; 43] {
        self.note.recipient().to_raw_address_bytes()
    }
    /// Exact atomic note value.
    pub fn value(&self) -> u64 {
        self.note.value().inner()
    }
    /// Encrypted memo recovered by this recipient's viewing key.
    pub fn memo(&self) -> &[u8; 512] {
        &self.memo
    }
    /// Recomputed commitment, independently equal to the transmitted commitment.
    pub fn commitment(&self) -> [u8; 32] {
        ExtractedNoteCommitment::from(self.note.commitment()).to_bytes()
    }
    /// Wallet-local spentness identifier, absent for incoming-viewing-key-only decryption.
    pub fn nullifier(&self) -> Option<[u8; 32]> {
        self.nullifier
    }
    /// Consume the retained opening with an authenticated complete witness.
    /// # Errors
    /// Rejects wrong ownership, malformed paths and stale/wrong anchors.
    pub fn into_spend(
        self,
        key: zeroize::Zeroizing<[u8; 32]>,
        position: u32,
        path: &[[u8; 32]; 32],
        anchor: [u8; 32],
    ) -> Result<OrchardSpendProverInputV1, OrchardSpendInputErrorV1> {
        spend_from_wallet_parts_v1(
            key,
            self.recipient(),
            self.value(),
            self.note.rho().to_bytes(),
            *self.note.rseed().as_bytes(),
            (position, path),
            anchor,
        )
    }
}

/// Decrypt only V3 outputs and explicitly recompute their commitments. The caller must first
/// authenticate the complete committed block and successful execution; decryption is not finality.
/// # Errors
/// Rejects invalid statement fields or inconsistent decrypted note commitments.
pub fn decrypt_outputs_v1(
    statement: &OrchardHalo2ActionsStatementV1,
    key: [u8; 32],
    internal: bool,
) -> Result<Vec<DecryptedIronwoodNoteV1>, OrchardSpendInputErrorV1> {
    let key = Zeroizing::new(key);
    let spending = Option::<SpendingKey>::from(SpendingKey::from_bytes(*key))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    let fvk = FullViewingKey::from(&spending);
    let incoming = Zeroizing::new(
        fvk.to_ivk(if internal {
            Scope::Internal
        } else {
            Scope::External
        })
        .to_bytes(),
    );
    let mut notes = decrypt_outputs_with_incoming_viewing_key_v1(statement, &incoming)?;
    for note in &mut notes {
        note.nullifier = Some(note.note.nullifier(&fvk).to_bytes());
    }
    Ok(notes)
}

/// Derive a scope-specific incoming viewing credential without exporting spending authority.
/// The returned raw credential is zeroized on drop; callers should persist it only in owner-only
/// native custody. Orchard's parsed viewing-key temporaries do not implement `Zeroize`.
/// # Errors
/// Rejects a noncanonical spending key.
pub fn incoming_viewing_key_v1(
    key: &[u8; 32],
    internal: bool,
) -> Result<Zeroizing<[u8; 64]>, OrchardSpendInputErrorV1> {
    let spending = Option::<SpendingKey>::from(SpendingKey::from_bytes(*key))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    Ok(Zeroizing::new(
        FullViewingKey::from(&spending)
            .to_ivk(if internal {
                Scope::Internal
            } else {
                Scope::External
            })
            .to_bytes(),
    ))
}

/// Derive a recipient using only a scope-specific incoming viewing credential.
/// # Errors
/// Rejects a noncanonical incoming viewing key.
pub fn recipient_from_incoming_viewing_key_v1(
    incoming: &[u8; 64],
    index: u32,
) -> Result<[u8; 43], OrchardSpendInputErrorV1> {
    let incoming = Option::<IncomingViewingKey>::from(IncomingViewingKey::from_bytes(incoming))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    Ok(incoming.address_at(index).to_raw_address_bytes())
}

/// Decrypt V3 outputs with incoming viewing authority only. This cannot derive nullifiers or
/// authorize spending. Encoded custody is borrowed, never copied into an unguarded key buffer.
/// The caller must first authenticate complete committed blocks and successful execution.
/// # Errors
/// Rejects malformed viewing material, statement fields or inconsistent note commitments.
pub fn decrypt_outputs_with_incoming_viewing_key_v1(
    statement: &OrchardHalo2ActionsStatementV1,
    incoming: &[u8; 64],
) -> Result<Vec<DecryptedIronwoodNoteV1>, OrchardSpendInputErrorV1> {
    use orchard::bundle::EffectsOnly;
    let incoming = Option::<IncomingViewingKey>::from(IncomingViewingKey::from_bytes(incoming))
        .ok_or(OrchardSpendInputErrorV1::SpendingKey)?;
    let mut actions = statement
        .actions
        .iter()
        .map(|a| {
            Action::from_parts(
                Option::<Nullifier>::from(Nullifier::from_bytes(&a.nullifier))
                    .ok_or(OrchardSpendInputErrorV1::Note)?,
                redpallas::VerificationKey::<SpendAuth>::try_from(a.randomized_key)
                    .map_err(|_| OrchardSpendInputErrorV1::Note)?,
                Option::<ExtractedNoteCommitment>::from(ExtractedNoteCommitment::from_bytes(
                    &a.note_commitment,
                ))
                .ok_or(OrchardSpendInputErrorV1::Note)?,
                TransmittedNoteCiphertext {
                    epk_bytes: a.ephemeral_key,
                    enc_ciphertext: orchard::note_encryption::NoteBytesData(
                        a.encrypted_note
                            .clone()
                            .try_into()
                            .map_err(|_| OrchardSpendInputErrorV1::Note)?,
                    ),
                    out_ciphertext: a
                        .outgoing_ciphertext
                        .clone()
                        .try_into()
                        .map_err(|_| OrchardSpendInputErrorV1::Note)?,
                },
                Option::<ValueCommitment>::from(ValueCommitment::from_bytes(&a.value_commitment))
                    .ok_or(OrchardSpendInputErrorV1::Note)?,
                (),
            )
            .map_err(|_| OrchardSpendInputErrorV1::Note)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actions.is_empty() || actions.len() > IRONWOOD_MAX_ACTIONS_V1 {
        return Err(OrchardSpendInputErrorV1::Note);
    }
    let first = actions.remove(0);
    let amount = i64::try_from(statement.value_balance.amount)
        .map_err(|_| OrchardSpendInputErrorV1::Note)?;
    let value = match statement.value_balance.direction {
        PrivacyValueBalanceDirectionV1::Balanced if amount == 0 => 0,
        PrivacyValueBalanceDirectionV1::IntoPool => -amount,
        PrivacyValueBalanceDirectionV1::OutOfPool => amount,
        _ => return Err(OrchardSpendInputErrorV1::Note),
    };
    let bundle = Bundle::from_parts(
        (first, actions).into(),
        Flags::ENABLED,
        value,
        Option::<Anchor>::from(Anchor::from_bytes(*statement.anchor.as_bytes()))
            .ok_or(OrchardSpendInputErrorV1::Note)?,
        EffectsOnly,
        BundleVersion::ironwood_v3(),
    )
    .map_err(|_| OrchardSpendInputErrorV1::Note)?;
    let mut notes = Vec::new();
    for (index, _, note, recipient, memo) in bundle.decrypt_outputs_with_keys(&[incoming]) {
        let commitment = ExtractedNoteCommitment::from(note.commitment()).to_bytes();
        if commitment != statement.actions[index].note_commitment || note.recipient() != recipient {
            return Err(OrchardSpendInputErrorV1::Note);
        }
        notes.push(DecryptedIronwoodNoteV1 {
            action_index: index,
            note,
            memo,
            nullifier: None,
        });
    }
    Ok(notes)
}

/// Reconstruct a depth-32 witness from an authenticated complete pool commitment prefix.
/// This is a local wallet calculation and grants no ledger authority to its inputs.
/// # Errors
/// Rejects an absent position, noncanonical commitments or an oversized local history page.
pub fn authentication_path_v1(
    commitments: &[[u8; 32]],
    position: u32,
) -> Result<([u8; 32], [[u8; 32]; 32]), OrchardSpendInputErrorV1> {
    use incrementalmerkletree::{Hashable as _, Level};
    let mut index = position as usize;
    if commitments.len() > 1_048_576 || index >= commitments.len() {
        return Err(OrchardSpendInputErrorV1::AuthenticationPathLength);
    }
    let mut nodes = commitments
        .iter()
        .map(|bytes| {
            Option::<MerkleHashOrchard>::from(MerkleHashOrchard::from_bytes(bytes))
                .ok_or(OrchardSpendInputErrorV1::Note)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut path = [[0; 32]; 32];
    for (height, sibling) in path.iter_mut().enumerate() {
        let level = Level::from(height as u8);
        let empty = MerkleHashOrchard::empty_root(level);
        *sibling = nodes.get(index ^ 1).unwrap_or(&empty).to_bytes();
        nodes = nodes
            .chunks(2)
            .map(|pair| MerkleHashOrchard::combine(level, &pair[0], pair.get(1).unwrap_or(&empty)))
            .collect();
        index /= 2;
    }
    Ok((nodes[0].to_bytes(), path))
}
