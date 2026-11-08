//! Source-selected ordinary Unload charge originals for suffix replay.
//! This child keeps preparation-plan fields private to their existing owner.

use super::*;
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1 as object_digest;

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    /// Read the selected ordinary plan and permanent request, never a caller's substitute.
    /// Suffix Unloads carry the signed quote in their own exact authority header instead.
    pub(in crate::kagemusha_wallet_state_v1) fn suffix_step_charge(
        &mut self,
        manifest: &manifest::Manifest,
        released: &ReleasedStep,
    ) -> Result<Option<ChargeOriginalsV1>, Error> {
        if released.suffix().is_some()
            || released.capsule().body().kind != KagemushaWalletOperationKindV1::Unload
        {
            return Ok(None);
        }
        released.verify_completion()?;
        let frozen = released.ordinary_frozen()?;
        let capsule = &frozen.capsule;
        let plan_address = manifest
            .capsule_plans
            .get(&mut self.archive, &released.capsule_digest())?
            .ok_or(Error::WitnessLost("suffix ordinary Unload plan mapping"))?;
        let plan_address: [u8; 32] = plan_address
            .as_slice()
            .try_into()
            .map_err(|_| Error::WitnessLost("suffix ordinary Unload plan address"))?;
        let plan_bytes = self.archive.read_object(&plan_address, PLAN_BOUND)?;
        if object_digest(&plan_bytes) != plan_address {
            return Err(Error::WitnessLost("suffix ordinary Unload plan original"));
        }
        let plan: Plan = archive::decode(&plan_bytes)?;
        let original = self
            .archive
            .read_object(&plan.request_object, REQUEST_MAX_BYTES)?;
        if object_digest(&original) != plan.request_object {
            return Err(Error::WitnessLost(
                "suffix ordinary Unload request original",
            ));
        }
        let (scheme, _) = self.proofs.ledger_scope()?;
        let intent = NativeIntentV1::decode(&original, &scheme)?;
        intent.validate(&scheme)?;
        plan.require(&self.scheme_id, &self.wallet_id, &intent, &original)?;
        let entry = self
            .preparation_entry(manifest, &intent.request_id())?
            .ok_or(Error::WitnessLost(
                "suffix ordinary Unload permanent request",
            ))?;
        entry.require_request(
            &mut self.archive,
            &self.scheme_id,
            &self.wallet_id,
            &intent,
            &original,
        )?;
        if entry.plan != Some(plan_address)
            || entry.request != Some(plan.request_object)
            || entry.capsule != Some(released.capsule_digest())
            || entry.operation != Some(released.operation_id())
            || entry.capsule_commitment != Some(capsule.receipt_commitment())
            || plan.source != capsule.body().predecessor_capsule_digest
        {
            return Err(Error::WitnessLost("suffix ordinary Unload selected plan"));
        }
        let request = intent
            .user_request()
            .ok_or(Error::WitnessLost("suffix ordinary Unload request kind"))?;
        let OperationActionV1::Unload { amount, charge } = &request.action else {
            return Err(Error::WitnessLost("suffix ordinary Unload request kind"));
        };
        if !matches!(capsule.body().statement.effect, KagemushaWalletEffectV1::Unload {amount: actual, ..} if actual == *amount)
        {
            return Err(Error::WitnessLost("suffix ordinary Unload amount"));
        }
        // Full quote/certificates/signature/ordinal/asset/effect checks belong to source_steps;
        // this read preserves their exact originals and their selected request provenance.
        Ok(charge.clone())
    }
}
