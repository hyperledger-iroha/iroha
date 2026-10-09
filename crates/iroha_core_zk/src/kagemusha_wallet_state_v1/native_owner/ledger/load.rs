//! Native Load finality through the wallet's selected ledger light client (R10 Load carve-out).
//!
//! The released wallet authenticates the exact receipt block's `CommitQC`, rooted in its
//! independently selected signed genesis, and the receipt's counted event inclusion before
//! any Load is accepted. The verified fact is retained durably, so retries and restarts never
//! depend on the light client staying at the receipt height. `Λ_load` binds the receipt terms;
//! it carries no finality proof.
use super::*;
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLoadReceiptV1,
    load_finality::{
        decode_kagemusha_wallet_load_event_path_v1,
        verify_finalized_kagemusha_wallet_load_event_v1,
    },
};
use iroha_crypto::MerkleProof;
use iroha_data_model::{events::EventBox, sumeragi_finality::VerifiedSumeragiBlock};

/// Durable native evidence that one exact receipt is included in its certified block.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::LoadConfirmationV1")]
pub(super) struct LoadConfirmation {
    version: u16,
    receipt_digest: [u8; 32],
    event_path_digest: [u8; 32],
    height: u64,
    block_hash: [u8; 32],
    event_index: u32,
}
impl LoadConfirmation {
    fn require(
        &self,
        receipt: &KagemushaWalletLoadReceiptV1,
        digest: &[u8; 32],
        event_path: &[u8],
    ) -> Result<LedgerProgressV1, Error> {
        if self.version != 1
            || self.receipt_digest != *digest
            || self.height != receipt.block_height
            || self.height < 2
            || self.block_hash == [0; 32]
        {
            return Err(Error::WitnessLost("retained Load confirmation"));
        }
        if self.event_path_digest != path_digest(event_path) {
            return Err(Error::OperationConflict);
        }
        Ok(LedgerProgressV1 {
            height: self.height,
            block_hash: self.block_hash,
        })
    }
}

fn path_digest(event_path: &[u8]) -> [u8; 32] {
    crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(event_path)
}

/// Exact canonical Load originals; decoding confers no finality.
struct LoadOriginals<'a> {
    receipt: KagemushaWalletLoadReceiptV1,
    digest: [u8; 32],
    path: MerkleProof<EventBox>,
    event_path: &'a [u8],
}
impl<'a> LoadOriginals<'a> {
    fn decode(
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        receipt: &[u8],
        event_path: &'a [u8],
    ) -> Result<Self, Error> {
        let decoded = valid(KagemushaWalletLoadReceiptV1::decode_canonical(receipt))?;
        let path = valid(decode_kagemusha_wallet_load_event_path_v1(event_path))?;
        if decoded.scheme_id != *scheme || decoded.wallet_id != *wallet {
            return Err(Error::Invalid("Load receipt wallet scope"));
        }
        Ok(Self {
            digest: valid(decoded.receipt_digest())?,
            receipt: decoded,
            path,
            event_path,
        })
    }
    fn from_capsule(
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        capsule: &'a KagemushaWalletRecoveryCapsuleV1,
    ) -> Result<Self, Error> {
        let one = |role| {
            let mut inputs = capsule
                .retained_inputs
                .iter()
                .filter(move |input| input.role == role);
            match (inputs.next(), inputs.next()) {
                (Some(input), None) => Ok(input.bytes.as_slice()),
                _ => Err(Error::Invalid("Load retained originals")),
            }
        };
        Self::decode(
            scheme,
            wallet,
            one(KagemushaWalletRetainedInputRoleV1::LoadReceipt)?,
            one(KagemushaWalletRetainedInputRoleV1::LoadEventPath)?,
        )
    }
}

/// Authenticate one receipt and event path against an already native-verified block.
fn confirm_in_block(
    verified: &VerifiedSumeragiBlock,
    network: iroha_data_model::NetworkId,
    chain: &str,
    load: &LoadOriginals<'_>,
) -> Result<LoadConfirmation, Error> {
    let event = verify_finalized_kagemusha_wallet_load_event_v1(
        verified,
        &load.path,
        network,
        chain,
        &load.receipt,
    )
    .map_err(|_| Error::Proof("Load event inclusion in the certified receipt block"))?;
    Ok(LoadConfirmation {
        version: 1,
        receipt_digest: load.digest,
        event_path_digest: path_digest(load.event_path),
        height: event.height(),
        block_hash: *event.block_hash().as_ref(),
        event_index: event.event_index(),
    })
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    fn retained_load_confirmation(
        &mut self,
        manifest: &manifest::Manifest,
        load: &LoadOriginals<'_>,
    ) -> Result<Option<LedgerProgressV1>, Error> {
        manifest
            .ledger_load_confirmations
            .get(&mut self.archive, &load.digest)?
            .map(|bytes| {
                archive::decode::<LoadConfirmation>(&bytes)?.require(
                    &load.receipt,
                    &load.digest,
                    load.event_path,
                )
            })
            .transpose()
    }

    /// Accept a Load only after native finality. A retained confirmation of the same receipt
    /// and event path is reused; otherwise the selected light-client tip must be exactly the
    /// receipt height, its retained `CommitQC` decision is reverified and the counted event
    /// inclusion is checked before the confirmation is durably published with `manifest`.
    /// # Errors
    /// Foreign or malformed originals, an absent light client, a tip other than the receipt
    /// height, failed certificate or inclusion checks, custody loss or publication failure.
    pub(in crate::kagemusha_wallet_state_v1) fn confirm_load_finality(
        &mut self,
        genesis: &SumeragiFinalityVerifier,
        selected: &mut [u8; 32],
        manifest: &mut manifest::Manifest,
        receipt: &[u8],
        event_path: &[u8],
    ) -> Result<LedgerProgressV1, Error> {
        let load = LoadOriginals::decode(&self.scheme_id, &self.wallet_id, receipt, event_path)?;
        if let Some(progress) = self.retained_load_confirmation(manifest, &load)? {
            return Ok(progress);
        }
        let (verifier, checkpoint) = self.selected_ledger(manifest, genesis)?;
        let checkpoint = checkpoint.ok_or(Error::Invalid("Load ledger light client absent"))?;
        match checkpoint.height().cmp(&load.receipt.block_height) {
            core::cmp::Ordering::Less => {
                return Err(Error::Invalid(
                    "Load receipt height is beyond the selected ledger tip",
                ));
            }
            core::cmp::Ordering::Greater => {
                return Err(Error::Invalid(
                    "selected ledger tip passed the unconfirmed Load receipt height",
                ));
            }
            core::cmp::Ordering::Equal => {}
        }
        let verified = verifier
            .verify_retained_decision(checkpoint.tip())
            .map_err(|_| Error::Proof("Load selected native decision"))?;
        // `selected_ledger` already bound this root to the installed scheme's network and chain.
        let confirmation = confirm_in_block(
            &verified,
            genesis.initial_epoch().network_id,
            genesis.chain_id(),
            &load,
        )?;
        manifest.ledger_load_confirmations = manifest.ledger_load_confirmations.set(
            &mut self.archive,
            load.digest,
            &archive::encode(&confirmation)?,
        )?;
        *selected = self.publish_manifest(*selected, manifest)?;
        Ok(LedgerProgressV1 {
            height: confirmation.height,
            block_hash: confirmation.block_hash,
        })
    }

    /// Require the durable native confirmation of a Load capsule's exact retained originals.
    /// No Load Advance can be performed without it, whatever path produced the capsule.
    /// # Errors
    /// Missing, mismatched or corrupt confirmation and malformed retained originals.
    pub(in crate::kagemusha_wallet_state_v1) fn require_load_confirmation(
        &mut self,
        manifest: &manifest::Manifest,
        capsule: &KagemushaWalletRecoveryCapsuleV1,
    ) -> Result<(), Error> {
        let load = LoadOriginals::from_capsule(&self.scheme_id, &self.wallet_id, capsule)?;
        self.retained_load_confirmation(manifest, &load)?
            .ok_or(Error::Invalid("Load native finality unconfirmed"))
            .map(|_| ())
    }
}

#[cfg(test)]
#[path = "load/tests.rs"]
mod tests;
