//! Issue and recover the exact payment-key authorization of the released Bootstrap.
use super::custody_archive::{Checkpoint, Objects, storage_error};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletSlotStatusV1 as Status,
    kagemusha_wallet_archive_object_digest_v1 as digest,
};
use crate::kagemusha_wallet_state_v1::{IndexRoot, ObjectStore};
use iroha_data_model::isi::kagemusha_wallet::spartan::ActivateKagemushaSpartanV1;
use iroha_data_model::kagemusha::{
    KagemushaDeviceSignatureV1, KagemushaWalletLedgerControlActionV1 as Action,
    KagemushaWalletLedgerControlBodyV1 as Body, KagemushaWalletLedgerControlV1 as Control,
};
use rand::rand_core::TryRng as _;

const MAX_ACTIVATION: usize = ActivateKagemushaSpartanV1::MAX_ACTIVATION_BYTES;
const _: () = assert!(MAX_ACTIVATION == native::MAX_ACTIVATION_CLAIM_BYTES);
fn lost(object: &'static str) -> ProviderError {
    ProviderError::UnavailableCustodyData { object }
}
fn invalid(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}

// The settlement index has disjoint, domain-separated keys for claims, attempts and activation.
fn plan_key(wallet: &[u8; 32]) -> [u8; 32] {
    digest(&[b"spartan-activation-plan-v1".as_slice(), wallet].concat())
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Plan {
    package: [u8; 32],
    nonce: [u8; 32],
    output: [u8; 32],
}
impl Plan {
    fn encode(&self) -> [u8; 96] {
        let mut bytes = [0; 96];
        bytes[..32].copy_from_slice(&self.package);
        bytes[32..64].copy_from_slice(&self.nonce);
        bytes[64..].copy_from_slice(&self.output);
        bytes
    }
    fn decode(bytes: &[u8]) -> Result<Self, ProviderError> {
        if bytes.len() != 96 || bytes[..32] == [0; 32] || bytes[32..64] == [0; 32] {
            return Err(lost("activation plan"));
        }
        Ok(Self {
            package: bytes[..32].try_into().expect("bounded package digest"),
            nonce: bytes[32..64].try_into().expect("bounded nonce"),
            output: bytes[64..].try_into().expect("bounded output address"),
        })
    }
    fn signature_address(&self, wallet: &[u8; 32]) -> [u8; 32] {
        digest(
            &[
                b"spartan-activation-signature-v1".as_slice(),
                wallet,
                &self.package,
                &self.nonce,
            ]
            .concat(),
        )
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    fn activation_plan(
        &mut self,
        slot: &Slot,
        wallet: &[u8; 32],
        checkpoint: &Checkpoint,
    ) -> Result<Option<Plan>, ProviderError> {
        self.provider.with_archive(slot, |archive| {
            IndexRoot(checkpoint.settlements)
                .get(&mut Objects(archive), &plan_key(wallet))
                .map_err(storage_error)?
                .as_deref()
                .map(Plan::decode)
                .transpose()
        })
    }
    fn publish_activation_plan(
        &mut self,
        slot: &Slot,
        wallet: &[u8; 32],
        previous: Option<Plan>,
        plan: Plan,
    ) -> Result<(), ProviderError> {
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, *wallet)?;
        if self.activation_plan(slot, wallet, &checkpoint)? != previous {
            return Err(invalid("activation plan changed"));
        }
        checkpoint.settlements = self.provider.with_archive(slot, |archive| {
            Ok(IndexRoot(checkpoint.settlements)
                .set(&mut Objects(archive), plan_key(wallet), &plan.encode())
                .map_err(storage_error)?
                .0)
        })?;
        self.publish_custody_checkpoint(slot, expected, &checkpoint)
    }
    fn read_activation_output(
        &mut self,
        slot: &Slot,
        plan: &Plan,
    ) -> Result<Vec<u8>, ProviderError> {
        if plan.output == [0; 32] {
            return Err(lost("signed activation original"));
        }
        self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .read_object(&plan.output, MAX_ACTIVATION)
                .map_err(storage_error)
        })
    }
    /// Read-only authority for matching an account-signed transaction to its exact local activation.
    pub(super) fn activation_original(&mut self, slot: &Slot) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let plan = self
            .activation_plan(slot, &wallet, &checkpoint)?
            .ok_or_else(|| lost("signed activation plan"))?;
        self.read_activation_output(slot, &plan)
    }

    /// Sign Activate in the ledger-control domain for the authentic local Bootstrap.
    /// Returns the scheme, asset, manifest digest and wallet (32 bytes each), followed by the
    /// exact bounded signed original. Retries across later heads return the same bytes.
    ///
    /// # Errors
    /// Requires monetary account authority, an intact released Bootstrap and its complete
    /// enrolled originals, an available payment key and a durable protected archive.
    pub fn issue_activation(&mut self, slot: &Slot) -> Result<Vec<u8>, ProviderError> {
        let original = self.issue_activation_original(slot)?;
        let asset = self.admitted_asset_digest(slot)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let mut response = Vec::with_capacity(128 + original.len());
        response.extend_from_slice(self.provider.scheme_id());
        response.extend_from_slice(&asset);
        response.extend_from_slice(self.parameters.manifest_digest());
        response.extend_from_slice(&wallet);
        response.extend_from_slice(&original);
        Ok(response)
    }

    fn issue_activation_original(&mut self, slot: &Slot) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let previous = self.activation_plan(slot, &wallet, &checkpoint)?;
        if let Some(plan) = previous.filter(|p| p.output != [0; 32]) {
            return self.read_activation_output(slot, &plan);
        }
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("activation requires released custody"));
        };
        let source = marker
            .head()
            .ok_or_else(|| lost("activation signing source"))?
            .2;
        let expected = self.bootstrap_operation_id(slot)?;
        let (capsule, retained) = self.provider.retained_transition(slot, &expected)?;
        if capsule.kind() != 1
            || capsule.wallet() != &wallet
            || capsule.operation_id() != &expected
            || retained.record.wallet() != &wallet
        {
            return Err(lost("activation historical Bootstrap bindings"));
        }
        let [account, asset, issuer] = self.activation_companions(slot)?;
        let package = self
            .engine
            .bootstrap_activation_package(
                &capsule,
                &retained.record,
                &account,
                &asset,
                &issuer,
                &self.parameters,
            )
            .map_err(provider_error)?;
        let plan = match previous {
            Some(plan) if &plan.package == package.digest() => plan,
            Some(_) => return Err(lost("activation Bootstrap package changed")),
            None => {
                let required = crate::kagemusha_wallet_state_v1::transfer_storage_required(
                    (MAX_ACTIVATION + 96 + 64) as u64,
                    3,
                );
                if self.provider.runtime_resources()?.storage_bytes < required {
                    return Err(ProviderError::NoSpace);
                }
                let mut nonce = [0; 32];
                rand::rngs::SysRng
                    .try_fill_bytes(&mut nonce)
                    .map_err(|_| invalid("activation nonce entropy"))?;
                if nonce == [0; 32] {
                    return Err(invalid("activation nonce entropy"));
                }
                let plan = Plan {
                    package: *package.digest(),
                    nonce,
                    output: [0; 32],
                };
                self.publish_activation_plan(slot, &wallet, None, plan)?;
                let (_, selected) = self.custody_checkpoint(slot, wallet)?;
                if self.activation_plan(slot, &wallet, &selected)? != Some(plan) {
                    return Err(lost("activation signing plan readback"));
                }
                plan
            }
        };
        let body = Body {
            version: 1,
            scheme_id: *capsule.scheme(),
            asset_digest: *capsule.asset(),
            wallet_id: wallet,
            action: Action::Activate {
                package_digest: plan.package,
            },
            nonce: plan.nonce,
        };
        let signature_key = plan.signature_address(&wallet);
        let read_signature =
            |archive: &crate::kagemusha_wallet_advance_v1::KagemushaWalletArchiveAccessV1<
                '_,
                F,
            >| {
                archive
                    .read_record(&signature_key, 64)
                    .map(|bytes| bytes.map(|b| b.as_slice().to_vec()))
            };
        let signature = match self.provider.with_archive(slot, read_signature)? {
            Some(bytes) => bytes,
            None => {
                let signed =
                    self.provider
                        .sign_activation(slot, source, marker.payment_key(), &body)?;
                self.provider.with_archive(slot, |archive| {
                    archive.write_record(&signature_key, signed.as_raw_bytes())
                })?;
                self.provider.drain_archive(slot)?;
                self.provider
                    .with_archive(slot, read_signature)?
                    .ok_or_else(|| lost("activation signature readback"))?
            }
        };
        let control = Control {
            body,
            signature: KagemushaDeviceSignatureV1::from_raw_bytes(&signature)
                .map_err(|_| lost("activation signature encoding"))?,
        };
        control
            .verify(marker.payment_key())
            .map_err(|_| lost("activation signature scope"))?;
        let bytes = package.authorize(&control).map_err(provider_error)?;
        let output = self.provider.with_archive(slot, |archive| {
            Objects(archive)
                .write_object(&bytes, MAX_ACTIVATION)
                .map_err(storage_error)
        })?;
        self.publish_activation_plan(slot, &wallet, Some(plan), Plan { output, ..plan })?;
        self.activation_original(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_signature_recovery_is_bound_to_exact_wallet_package_and_nonce() {
        let p = Plan {
            package: [1; 32],
            nonce: [2; 32],
            output: [0; 32],
        };
        let address = p.signature_address(&[3; 32]);
        for changed in [
            Plan {
                package: [4; 32],
                ..p
            },
            Plan {
                nonce: [4; 32],
                ..p
            },
        ] {
            assert_ne!(address, changed.signature_address(&[3; 32]));
        }
        assert_ne!(address, p.signature_address(&[4; 32]));
        assert_ne!(address, plan_key(&[3; 32]));
        assert!(Plan::decode(&p.encode()).unwrap() == p);
        assert!(Plan::decode(&[0; 96]).is_err());
        assert!(Plan::decode(&p.encode()[..95]).is_err());
    }
}
