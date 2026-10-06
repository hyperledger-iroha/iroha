//! Hard binding of an Archive's retained Payment and pending descriptor.
//!
//! Historical Send proofs are opaque here: their exact original bytes are
//! hashed into the Payment that the incoming evidence must acknowledge. This
//! owner does not replace incoming evidence verification or pending membership.

use ff::Field;
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{
    GlueChip, UintChip, Word,
    bytes::{tape::BytesChip, variable::ActiveBytes},
};
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierChip};

use super::require_task;
use crate::{
    a_relation::{
        context::{ContextInputs, ContextObjectCells, ContextObjectSpec, ContextPlan},
        own::OwnPolicy,
        schedule::OperationTask,
    },
    operation_relation::{
        incoming_statement::{IncomingStatementCells, StatementView},
        objects::{
            ObjectKind, SignedObjectCells,
            credential::CredentialCells,
            payment::{PaymentCells, PaymentInputs},
            request::RequestCells,
        },
    },
};

const KINDS: [ObjectKind; 4] = [
    ObjectKind::Request,
    ObjectKind::Credential,
    ObjectKind::Receipt,
    ObjectKind::Credential,
];

pub(super) const DESCRIPTOR_SLOT: usize = 11;
pub(super) const DESCRIPTOR_SPEC: ContextObjectSpec = ContextObjectSpec {
    tag: 12,
    capacity: 7 * 32,
};

/// Exact signed sources and transitive Payment transcript retained after Send.
#[derive(Clone, Copy)]
pub struct ArchiveRetainedSources<'a> {
    /// Request, payer credential, Send receipt and quoted receiver credential.
    /// Each includes its original raw signature after the fixed body.
    pub signed: [&'a [Value<u8>]; 4],
    /// Canonical163-byte Payment transcript derived from the retained frame.
    pub payment: &'a [Value<u8>],
}

/// Original historical Send statement and the two exact opaque proof tapes.
#[derive(Clone, Copy)]
pub struct ArchiveRetainedInputs<'a> {
    /// All26 original Send statement words, including the pending descriptor.
    pub statement: &'a [Word<Fp>; 26],
    /// Exact raw Omega carrier used by the native consuming proof digest.
    pub omega: &'a ActiveBytes<Fp>,
    /// Exact original sigma bytes, without fixed-descriptor padding.
    pub sigma: &'a ActiveBytes<Fp>,
}

/// Hard original Payment binding shared by every evidence and removal owner.
#[derive(Clone, Debug)]
pub struct ArchiveRetainedPayment {
    request: RequestCells,
    receiver: CredentialCells,
    payment: PaymentCells,
    statement: IncomingStatementCells,
    context: [ContextObjectCells; 9],
    specs: [ContextObjectSpec; 9],
}

impl ArchiveRetainedPayment {
    /// Fixed tags4..12 following Archive's three own authorization objects.
    /// Slots are four signed sources, Payment, Send words, raw Omega, raw sigma
    /// and the seven-word pending descriptor. Proof capacities are key-fixed.
    /// # Errors
    /// Empty/overflowing proof capacity or impossible fixed-size conversion.
    pub fn context_specs(
        omega_capacity: usize,
        sigma_capacity: usize,
    ) -> Result<[ContextObjectSpec; 9], Error> {
        if omega_capacity == 0 || sigma_capacity == 0 {
            return Err(Error::Synthesis);
        }
        let mut capacities = KINDS.map(|kind| kind.body_len() + 64).to_vec();
        capacities.extend([
            PaymentCells::BYTES,
            26 * 32,
            omega_capacity,
            sigma_capacity,
            7 * 32,
        ]);
        capacities
            .into_iter()
            .enumerate()
            .map(|(i, capacity)| {
                Ok(ContextObjectSpec {
                    tag: u32::try_from(i + 4).map_err(|_| Error::BoundsFailure)?,
                    capacity: u32::try_from(capacity).map_err(|_| Error::BoundsFailure)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }

    /// Derive every retained commitment from the same original tapes.
    /// This hard own-data binding runs even when incoming evidence is invalid.
    /// No signature or proof verdict is inferred from these historical bytes.
    /// # Errors
    /// Wrong source size/schema, malformed retained objects, changed quoted
    /// credential or inconsistent Payment/Send component addresses.
    pub fn from_sources(
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        policy: OwnPolicy,
        sources: ArchiveRetainedSources<'_>,
        input: ArchiveRetainedInputs<'_>,
    ) -> Result<Self, Error> {
        let specs = Self::context_specs(input.omega.run().len(), input.sigma.run().len())?;
        let mut objects = Vec::new();
        let mut context = Vec::new();
        for ((kind, source), spec) in KINDS.into_iter().zip(sources.signed).zip(specs) {
            let run = bytes.run(
                region,
                source,
                &kind.primary_segments(),
                &kind.secondary_segments(),
            )?;
            let lanes = chip.operation_lanes()?;
            let object = SignedObjectCells::from_run(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                kind,
                &run,
            )?;
            context.push(ContextObjectCells::from_exact_run(
                chip,
                region,
                spec,
                object.digest(),
                &run,
            )?);
            objects.push(object);
        }
        let scope = policy.scope(chip, region)?;
        let run = bytes.run(
            region,
            sources.payment,
            &PaymentCells::primary_segments(),
            &PaymentCells::secondary_segments(),
        )?;
        let lanes = chip.operation_lanes()?;
        let mut uint = UintChip::new(lanes.glue, lanes.range);
        let left = input.omega.packed().length_prefixed(&mut uint, region)?;
        let right = input.sigma.packed().length_prefixed(&mut uint, region)?;
        let proof_digest = left.concat(&mut uint, region, &right)?.digest(
            &mut uint,
            lanes.hash.sponge_mut()?,
            region,
            u64::from_le_bytes(*b"kgwprf_1"),
        )?;
        let statement = IncomingStatementCells::constrain(
            &mut uint,
            lanes.hash,
            region,
            Variant::Send,
            input.statement,
        )?;
        let request = RequestCells::check(&mut uint, lanes.hash, region, &objects[0])?;
        let payer = CredentialCells::check(&mut uint, region, &objects[1])?;
        let receiver = CredentialCells::check(&mut uint, region, &objects[3])?;
        GlueChip::assert_equal(
            region,
            request.object().word(8)?,
            receiver.object().digest(),
        )?;
        GlueChip::assert_constant(region, receiver.valid().word(), Fp::ONE)?;
        for (request_index, credential_index) in [(1, 1), (2, 2), (5, 3), (6, 4)] {
            for (a, b) in request
                .object()
                .identifier(request_index)?
                .iter()
                .zip(receiver.object().identifier(credential_index)?)
            {
                GlueChip::assert_equal(region, a, b)?;
            }
        }
        let payment = PaymentCells::from_run(
            &mut uint,
            lanes.hash,
            region,
            &run,
            &PaymentInputs {
                request: &request,
                payer: &payer,
                statement: &statement,
                receipt: &objects[2],
                provider: &scope.provider,
                proof_digest: &proof_digest,
            },
        )?;
        GlueChip::assert_constant(region, payment.valid().word(), Fp::ONE)?;
        context.push(ContextObjectCells::from_exact_run(
            chip,
            region,
            specs[4],
            payment.digest(),
            &run,
        )?);
        context.push(ContextObjectCells::from_internal_words(
            chip,
            region,
            specs[5],
            statement.fields(),
        )?);
        for (spec, carrier) in [(specs[6], input.omega), (specs[7], input.sigma)] {
            context.push(ContextObjectCells::from_active(
                chip,
                region,
                spec,
                &proof_digest,
                carrier,
            )?);
        }
        context.push(ContextObjectCells::from_internal_words(
            chip,
            region,
            specs[8],
            &statement.fields()[17..24],
        )?);
        Ok(Self {
            request,
            receiver,
            payment,
            statement,
            context: context.try_into().map_err(|_| Error::Synthesis)?,
            specs,
        })
    }

    /// Same original nine commitments, placed after the three own objects.
    pub const fn context(&self) -> &[ContextObjectCells; 9] {
        &self.context
    }

    /// Original held Request used by both forms of incoming evidence.
    pub const fn request(&self) -> &RequestCells {
        &self.request
    }

    /// Original quoted receiver credential; current receiver renewal is allowed.
    pub const fn receiver(&self) -> &CredentialCells {
        &self.receiver
    }

    /// Digest of the exact retained Payment, including both opaque proof tapes.
    pub const fn payment(&self) -> &PaymentCells {
        &self.payment
    }

    /// Bind this hard owner to the common context, own wallet and credit.
    /// The terminal Effects owner must authenticate both removals using the
    /// descriptor committed at slot11. Incoming evidence may not gate this task.
    /// # Errors
    /// Wrong owner/schema, another wallet/credit or substituted retained source.
    pub fn bind_context(
        &self,
        region: &mut Region<'_, Fp>,
        plan: &ContextPlan,
        stage: u32,
        input: &ContextInputs<'_>,
    ) -> Result<(), Error> {
        require_task(plan, stage, OperationTask::ArchiveRetainedPayment)?;
        if plan.object_specs().get(3..12) != Some(self.specs.as_slice())
            || input.objects.len() != plan.object_specs().len()
            || input.own_statement.variant() != plan.operation().frame().variant()
        {
            return Err(Error::Synthesis);
        }
        let predecessor = input.predecessor.ok_or(Error::Synthesis)?;
        for (a, b) in self
            .request
            .object()
            .identifier(3)?
            .iter()
            .zip(&predecessor.public.fields()[6..8])
        {
            GlueChip::assert_equal(region, a, b)?;
        }
        for (a, b) in self.statement.fields()[1..7]
            .iter()
            .zip(&input.own_statement.fields()[1..7])
        {
            GlueChip::assert_equal(region, a, b)?;
        }
        GlueChip::assert_equal(
            region,
            self.request.credit_id(),
            &input.own_statement.fields()[17],
        )?;
        for (actual, expected) in self.context.iter().zip(&input.objects[3..12]) {
            for (a, b) in actual
                .commitment_words()
                .iter()
                .zip(expected.commitment_words())
            {
                GlueChip::assert_equal(region, a, &b)?;
            }
        }
        Ok(())
    }
}
