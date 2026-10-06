//! Send's current-credential, exact Request, held-fee and depth32 map composition.
//!
//! The hard predecessor authenticates the held credential and fee-schedule
//! digests. Send has no local signature Q: the receiver owns its Request
//! signature obligation, and issuer authorization occurred at installation.
//! The complete A schedule must also hard-verify the exact control-selected
//! sigma and retain all predecessor/Q opening obligations.

use ff::Field;
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{GlueChip, UintChip, bytes::tape::BytesChip};
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierChip};

use super::{
    SigmaBindingCells,
    context::{ContextObjectCells, ContextObjectSpec},
};
use crate::operation_relation::{
    map_effects::{InsertCells, MapEffectsChip, MapState, MapTransition},
    objects::{
        ObjectKind, SignedObjectCells, credential::CredentialCells, fee, policy::PolicyCells,
        request::RequestCells,
    },
};

/// Exact payer credential, signed Request and fixed held-fee slot, in that order.
#[derive(Clone, Debug)]
pub struct SendObjects {
    objects: [SignedObjectCells; 3],
    context: [ContextObjectCells; 3],
}

/// The exact state and sigma cells retained by the complete A schedule.
#[derive(Clone, Copy, Debug)]
pub struct SendInputs<'a> {
    /// State and public prefix authenticated by the hard predecessor proof.
    pub predecessor: MapState<'a>,
    /// Successor state and public prefix committed by the operation.
    pub successor: MapState<'a>,
    /// Own statement and exact sigma bytes bound to the hard `Q_sigma` output.
    pub sigma: &'a SigmaBindingCells,
}

/// Same-cell ownership/Request/fee facts, retaining their exact state transition.
///
/// A fixed complete stage schedule must execute both `pending` and
/// `fee_and_unchanged`, possibly in different proofs that rebind the same context.
/// Constructing this value alone does not establish the operation's map effects.
#[derive(Clone, Debug)]
pub struct SendCells<'a> {
    input: SendInputs<'a>,
    fee_schedule: iroha_plonk_gadgets::Word<Fp>,
}
impl SendCells<'_> {
    fn transition(&self) -> Result<MapTransition<'_>, Error> {
        Ok(MapTransition {
            statement: self.input.sigma.hard_statement()?,
            predecessor: self.input.predecessor,
            successor: self.input.successor,
        })
    }
    /// Check the exact pending descriptor insertion into the adjusted lineage map.
    ///
    /// # Errors
    /// Layout failure or invalid authenticated path/descriptor/root.
    pub fn pending(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        path: &InsertCells,
    ) -> Result<(), Error> {
        let lanes = chip.operation_lanes()?;
        MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash).send_pending(
            region,
            &self.transition()?,
            path,
        )
    }
    /// Check the fee insert/no-op and every untouched map and adjusted balance.
    ///
    /// The schedule digest comes only from this same authenticated Request.
    /// # Errors
    /// Layout failure or invalid fee path/root/unchanged fields.
    pub fn fee_and_unchanged(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        path: &InsertCells,
    ) -> Result<(), Error> {
        let lanes = chip.operation_lanes()?;
        MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash).send_fee_and_unchanged(
            region,
            &self.transition()?,
            path,
            &self.fee_schedule,
        )
    }
}

impl SendObjects {
    /// Fixed context categories, including every original signature byte.
    ///
    /// # Errors
    /// An impossible fixed-schema capacity conversion.
    pub fn context_specs() -> Result<[ContextObjectSpec; 3], Error> {
        [
            ObjectKind::Credential,
            ObjectKind::Request,
            ObjectKind::FeeSchedule,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| -> Result<_, Error> {
            Ok(ContextObjectSpec {
                tag: u32::try_from(index + 1).map_err(|_| Error::BoundsFailure)?,
                capacity: u32::try_from(kind.body_len() + 64).map_err(|_| Error::BoundsFailure)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Synthesis)
    }

    /// Decode one tape per object and commit those identical bytes to context.
    ///
    /// The fee slot is total: a head with no held schedule permits a malformed
    /// fixed dummy, whose original digest still enters context. Credential and
    /// Request structure are hard own-operation requirements.
    ///
    /// # Errors
    /// Wrong fixed shape or layout failure; malformed own objects are unsatisfiable.
    pub fn decode(
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        sources: [&[Value<u8>]; 3],
    ) -> Result<Self, Error> {
        let mut objects = Vec::new();
        let mut context = Vec::new();
        for ((kind, source), spec) in [
            ObjectKind::Credential,
            ObjectKind::Request,
            ObjectKind::FeeSchedule,
        ]
        .into_iter()
        .zip(sources)
        .zip(Self::context_specs()?)
        {
            let run = bytes.run(
                region,
                source,
                &kind.primary_segments(),
                &kind.secondary_segments(),
            )?;
            let lanes = chip.operation_lanes()?;
            let mut uint = UintChip::new(lanes.glue, lanes.range);
            let object = if kind == ObjectKind::FeeSchedule {
                SignedObjectCells::decode_soft(&mut uint, lanes.hash, region, kind, &run)?.0
            } else {
                SignedObjectCells::from_run(&mut uint, lanes.hash, region, kind, &run)?
            };
            context.push(ContextObjectCells::from_exact_run(
                chip,
                region,
                spec,
                object.digest(),
                &run,
            )?);
            objects.push(object);
        }
        Ok(Self {
            objects: objects.try_into().map_err(|_| Error::Synthesis)?,
            context: context.try_into().map_err(|_| Error::Synthesis)?,
        })
    }

    /// Original same-tape commitments for every stage of the fixed schedule.
    pub const fn context(&self) -> &[ContextObjectCells; 3] {
        &self.context
    }

    /// Bind current ownership, exact credit/fees and both depth32 insertions.
    ///
    /// The fee-map value uses this same Request's historical schedule digest;
    /// callers cannot supply an independently chosen fee schedule. The exact
    /// control-selected sigma remains a separate hard Q obligation of A.
    ///
    /// # Errors
    /// Wrong variant or layout failure; wrong ownership, fees or paths reject.
    pub fn constrain(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: SendInputs<'_>,
        pending: &InsertCells,
        fee_path: &InsertCells,
    ) -> Result<(), Error> {
        let cells = self.authenticate(chip, region, input)?;
        cells.pending(chip, region, pending)?;
        cells.fee_and_unchanged(chip, region, fee_path)
    }

    /// Check current ownership and exact Request/held-fee terms on the same tapes.
    ///
    /// The returned cells retain the exact transition for separate fixed-stage
    /// map checks; they cannot be assembled from unchecked host verdicts.
    /// # Errors
    /// Wrong operation class or layout failure; false ownership/fees reject.
    pub fn authenticate<'a>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: SendInputs<'a>,
    ) -> Result<SendCells<'a>, Error> {
        let statement = input.sigma.hard_statement()?;
        if statement.variant() != Variant::Send {
            return Err(Error::Synthesis);
        }
        let lanes = chip.operation_lanes()?;
        let mut uint = UintChip::new(lanes.glue, lanes.range);
        let payer = CredentialCells::check(&mut uint, region, &self.objects[0])?;
        let valid = payer.bind_current(
            &mut uint,
            region,
            input.predecessor.state,
            input.predecessor.lineage,
        )?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        let request = RequestCells::check(&mut uint, lanes.hash, region, &self.objects[1])?;
        let valid = request.bind_send(&mut uint, region, statement, &payer)?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        let schedule = PolicyCells::check(&mut uint, region, &self.objects[2])?;
        let valid = fee::bind_send(
            &mut uint,
            region,
            &request,
            input.predecessor.state,
            &schedule,
        )?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        Ok(SendCells {
            input,
            fee_schedule: request.object().word(10)?.clone(),
        })
    }
}
