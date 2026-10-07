//! Genuine one-task A/W restoration and final Omega; no alternate proof engine.
use crate::kagemusha_wallet_artifacts_v1::{
    InstalledVerifierPackV1,
    producer_inventory::{
        OriginalSourceV1, QualifiedOperationOwnerV1, QualifiedOperationRouteV1,
        QualifiedWalletSourcesV1,
    },
};
use crate::kagemusha_wallet_state_v1::{Cancellation, CheckpointLayout, Error};
use ff::{Field, PrimeField};
use iroha_data_model::kagemusha::kagemusha_wallet_v1::{
    KagemushaWalletLineagePublicV1, KagemushaWalletLineageV1,
};
use iroha_kagemusha_proof::a_relation::native::{
    archive, bootstrap, consuming, load, receive, refresh, send,
};
use iroha_kagemusha_proof::a_relation::schedule::compiled::OperationRoute;
use iroha_kagemusha_proof::omega::native as omega;
use iroha_pasta::{Fp, Fq, msm::MemoryBudget};
use iroha_plonk::keys::pk::artifact::ReadConfig;
use iroha_plonk::{ProverConfig, ProverRandomness};
use iroha_plonk_recursion::FoldConfig;
use rand_core_06::{OsRng, RngCore};

pub(crate) enum Inputs {
    Bootstrap(bootstrap::Inputs),
    Load(load::Inputs),
    Send(send::Inputs),
    Receive(receive::Inputs),
    Consuming(consuming::Inputs),
    Archive(archive::Inputs),
    Refresh(refresh::Inputs),
}
fn layout(
    descriptor: &[u8; 32],
    key: &[u8; 32],
    payload: usize,
) -> Result<CheckpointLayout, Error> {
    let mut bytes = descriptor.to_vec();
    bytes.extend_from_slice(key);
    Ok(CheckpointLayout {
        artifact_digest: crate::kagemusha_wallet_advance_v1::kagemusha_wallet_provider_digest_v1(
            "wallet-native-aw-layout",
            &bytes,
        ),
        payload_bytes: u32::try_from(payload).map_err(|_| Error::Proof("A/W payload"))?,
    })
}
macro_rules! layouts {
    ($producer:expr) => {
        $producer
            .checkpoint_layouts()
            .map_err(|_| Error::Proof("native installed schedule"))?
            .iter()
            .map(|value| {
                layout(
                    value.descriptor_digest(),
                    value.verifying_key_digest(),
                    value.payload_bytes(),
                )
            })
            .collect::<Result<Vec<_>, Error>>()
    };
}
pub(crate) fn schedule(owner: &QualifiedOperationRouteV1) -> Result<Vec<CheckpointLayout>, Error> {
    Ok(match owner.owner() {
        QualifiedOperationOwnerV1::Bootstrap(producer) => producer
            .prover()
            .complete_checkpoint_layouts()
            .map_err(|_| Error::Proof("Bootstrap installed schedule"))?
            .iter()
            .map(|value| {
                layout(
                    value.descriptor_digest(),
                    value.verifying_key_digest(),
                    value.payload_bytes(),
                )
            })
            .collect::<Result<Vec<_>, Error>>()?,
        QualifiedOperationOwnerV1::Load(p) => layouts!(p)?,
        QualifiedOperationOwnerV1::Send(p) => layouts!(p)?,
        QualifiedOperationOwnerV1::Receive(p) => layouts!(p)?,
        QualifiedOperationOwnerV1::Consuming(p) => layouts!(p)?,
        QualifiedOperationOwnerV1::Archive(p) => layouts!(p)?,
        QualifiedOperationOwnerV1::Refresh(p) => layouts!(p)?,
    })
}
/// One genuine stage result. It grants no durable completion or Advance authority.
pub(crate) enum StageProgressV1 {
    /// Original verified A/W checkpoint to durably publish before another stage.
    Checkpoint(Vec<u8>),
    /// Sole final Omega, fully admitted by the authenticated installed verifier.
    Complete(KagemushaWalletLineageV1),
}

/// The next original key follows the exact native A/W order; no key is needed to restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NextKey {
    A(usize),
    W(usize),
    Omega,
}
fn next_key(completed: usize, count: usize) -> Result<NextKey, Error> {
    if count == 0 || count % 2 == 0 || completed > count {
        return Err(Error::WitnessLost("native stage ordinal"));
    }
    Ok(if completed == count {
        NextKey::Omega
    } else if completed % 2 == 0 {
        NextKey::A(completed / 2)
    } else {
        NextKey::W(completed / 2)
    })
}

fn complete(
    installed: &InstalledVerifierPackV1,
    sources: &QualifiedWalletSourcesV1,
    originals: &mut dyn OriginalSourceV1,
    read: ReadConfig,
    owner: &QualifiedOperationRouteV1,
    proof: Vec<u8>,
    frame: Vec<Fp>,
    pallas: iroha_plonk_recursion::AccumulatorT<iroha_pasta::Ep>,
    public: KagemushaWalletLineagePublicV1,
    cancellation: &Cancellation,
    budget: MemoryBudget,
) -> Result<StageProgressV1, Error> {
    cancellation.check()?;
    let key = sources.omega().key();
    let digest = key
        .key()
        .kagemusha_digest(key.binding())
        .map_err(|_| Error::Proof("native installed Omega identity"))?
        .to_repr();
    let public_fields = crate::kagemusha_wallet_proofs_v1::lineage_public_fields(&public, digest)
        .map_err(|_| Error::Invalid("native final lineage prefix"))?;
    // All source A/W PKs have been dropped. Import exactly the sole final original now.
    let final_owner = sources
        .import_omega(originals, read)
        .map_err(|_| Error::Proof("native final original Omega source"))?;
    let mut salt = [0; 32];
    OsRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| Error::Proof("native fold entropy"))?;
    let session = final_owner
        .prepare(
            omega::Input {
                key: owner.terminal().key().clone(),
                proof,
                frame: frame
                    .try_into()
                    .map_err(|_| Error::Proof("native terminal frame"))?,
                public: public_fields,
                pallas,
            },
            salt,
            budget,
        )
        .map_err(|_| Error::Proof("native final Omega prepare"))?;
    cancellation.check()?;
    let result = session
        .prove(
            ProverRandomness::hedged(),
            ProverConfig { msm_budget: budget },
        )
        .map_err(|_| Error::Proof("native final Omega proof and decisions"))?;
    cancellation.check()?;
    let lineage = KagemushaWalletLineageV1 {
        public,
        proof: result.transport(),
    };
    // Authentic seventeen-verifier owner verifies the final proof and both transported claims.
    installed
        .verifier()
        .verify_lineage(&lineage, budget)
        .map_err(|_| Error::Proof("native complete installed Omega admission"))?;
    cancellation.check()?;
    Ok(StageProgressV1::Complete(lineage))
}
macro_rules! keyed_stage {
    (source_first, $method:ident, $session:expr, $source:expr, $key:expr, $salt:expr, $fold:expr, $randomness:expr, $config:expr $(,)?) => {
        $session.$method($source, $key, $salt, $fold, $randomness, $config)
    };
    (key_first, $method:ident, $session:expr, $source:expr, $key:expr, $salt:expr, $fold:expr, $randomness:expr, $config:expr $(,)?) => {
        $session.$method($key, $source, $salt, $fold, $randomness, $config)
    };
}
macro_rules! run_staged {
    ($order:ident,$producer:expr,$input:expr,$owner:expr,$public:expr,$checkpoints:expr,$cancel:expr,$budget:expr,$installed:expr,$sources:expr,$route:expr,$originals:expr,$read:expr) => {{
        let producer = $producer;
        let checkpoints = $checkpoints;
        let cancellation = $cancel;
        let budget = $budget;
        let count = producer
            .checkpoint_layouts()
            .map_err(|_| Error::Proof("native installed schedule"))?
            .len();
        next_key(checkpoints.len(), count)?;
        if checkpoints.len() > count {
            return Err(Error::WitnessLost("extra native source checkpoint"));
        }
        cancellation.check()?;
        let session = producer
            .prepare($input, budget)
            .map_err(|_| Error::Proof("native typed operation prepare"))?;
        let config = ProverConfig { msm_budget: budget };
        let fold = FoldConfig {
            kernel_budget: budget,
            ..FoldConfig::default()
        };
        let mut current_a = None;
        let mut current_w = None;
        for (index, bytes) in checkpoints.iter().enumerate() {
            cancellation.check()?;
            if index == 0 {
                current_a = Some(
                    session
                        .restore_first_checkpoint(bytes, budget)
                        .map_err(|_| Error::Proof("native A1 original restore"))?,
                );
            } else if index % 2 == 1 {
                current_w = Some(
                    session
                        .restore_wrapper_checkpoint(
                            current_a
                                .as_ref()
                                .ok_or(Error::WitnessLost("native prior A source"))?,
                            bytes,
                            budget,
                        )
                        .map_err(|_| Error::Proof("native W original restore"))?,
                );
                current_a = None;
            } else {
                current_a = Some(
                    session
                        .restore_a_checkpoint(
                            current_w
                                .as_ref()
                                .ok_or(Error::WitnessLost("native prior W source"))?,
                            bytes,
                            budget,
                        )
                        .map_err(|_| Error::Proof("native A original restore"))?,
                );
                current_w = None;
            }
        }
        cancellation.check()?;
        if checkpoints.len() < count {
            let bytes = if checkpoints.is_empty() {
                cancellation.check()?;
                let key = $sources
                    .import_a($route, 0, $originals, $read)
                    .map_err(|_| Error::Proof("native first original A source"))?;
                cancellation.check()?;
                let a = session
                    .first(
                        &key,
                        Fp::random(OsRng),
                        &fold,
                        ProverRandomness::hedged(),
                        config,
                    )
                    .map_err(|_| Error::Proof("native A1 proof"))?;
                session
                    .encode_a_checkpoint(&a, budget)
                    .map_err(|_| Error::Proof("native A1 checkpoint"))?
            } else if checkpoints.len() % 2 == 1 {
                cancellation.check()?;
                let key = $sources
                    .import_w($route, checkpoints.len() / 2, $originals, $read)
                    .map_err(|_| Error::Proof("native original W source"))?;
                cancellation.check()?;
                let w = keyed_stage!(
                    $order,
                    wrapper,
                    session,
                    current_a
                        .as_ref()
                        .ok_or(Error::WitnessLost("native prior A"))?,
                    &key,
                    Fq::random(OsRng),
                    &fold,
                    ProverRandomness::hedged(),
                    config,
                )
                .map_err(|_| Error::Proof("native W proof"))?;
                session
                    .encode_wrapper_checkpoint(&w, budget)
                    .map_err(|_| Error::Proof("native W checkpoint"))?
            } else {
                cancellation.check()?;
                let key = $sources
                    .import_a($route, checkpoints.len() / 2, $originals, $read)
                    .map_err(|_| Error::Proof("native next original A source"))?;
                cancellation.check()?;
                let a = keyed_stage!(
                    $order,
                    advance,
                    session,
                    current_w
                        .as_ref()
                        .ok_or(Error::WitnessLost("native prior W"))?,
                    &key,
                    Fp::random(OsRng),
                    &fold,
                    ProverRandomness::hedged(),
                    config,
                )
                .map_err(|_| Error::Proof("native A proof"))?;
                session
                    .encode_a_checkpoint(&a, budget)
                    .map_err(|_| Error::Proof("native A checkpoint"))?
            };
            cancellation.check()?;
            Ok(StageProgressV1::Checkpoint(bytes))
        } else {
            let terminal = session
                .terminal(
                    current_a
                        .as_ref()
                        .ok_or(Error::WitnessLost("native terminal A source"))?,
                    budget,
                )
                .map_err(|_| Error::Proof("native terminal A verification"))?;
            complete(
                $installed,
                $sources,
                $originals,
                $read,
                $owner,
                terminal.proof,
                terminal.instances,
                terminal.pallas,
                $public,
                cancellation,
                budget,
            )
        }
    }};
}
/// Perform one real original-key A/W task, or the sole final Omega after restoring every A/W.
/// Typed inputs and public prefix must come from canonical native preparation of the released step.
/// This component receives no slot or scheme identifiers from the foreign wallet-open boundary.
pub(crate) fn next(
    installed: &InstalledVerifierPackV1,
    sources: &QualifiedWalletSourcesV1,
    route: OperationRoute,
    originals: &mut dyn OriginalSourceV1,
    read: ReadConfig,
    input: Inputs,
    public: KagemushaWalletLineagePublicV1,
    checkpoints: &[Vec<u8>],
    cancellation: &Cancellation,
    budget: MemoryBudget,
) -> Result<StageProgressV1, Error> {
    cancellation.check()?;
    if sources.installation()
        != (
            installed.verifier().scheme().scheme_id(),
            installed.verifier().manifest_digest(),
        )
        || public.scheme_id != sources.installation().0
        || public.relation_id != installed.verifier().scheme().relation_id
    {
        return Err(Error::Invalid("native source-qualified installation"));
    }
    public
        .validate()
        .map_err(|_| Error::Invalid("native lineage public"))?;
    let owner = sources
        .route(route)
        .map_err(|_| Error::Proof("native qualified logical route"))?;
    match (owner.owner(), input) {
        (QualifiedOperationOwnerV1::Bootstrap(owner_bootstrap), Inputs::Bootstrap(input)) => {
            let producer = owner_bootstrap.prover();
            next_key(checkpoints.len(), 3)?;
            let session = producer
                .prepare(input, budget)
                .map_err(|_| Error::Proof("Bootstrap typed prepare"))?;
            let config = ProverConfig { msm_budget: budget };
            let fold = FoldConfig {
                kernel_budget: budget,
                ..FoldConfig::default()
            };
            if checkpoints.is_empty() {
                cancellation.check()?;
                let key = sources
                    .import_a(route, 0, originals, read)
                    .map_err(|_| Error::Proof("Bootstrap A1 original"))?;
                cancellation.check()?;
                let a = session
                    .first(&key, ProverRandomness::hedged(), config)
                    .map_err(|_| Error::Proof("Bootstrap A1 prove"))?;
                let bytes = session
                    .encode_first_checkpoint(&a, budget)
                    .map_err(|_| Error::Proof("Bootstrap A1 retain"))?;
                cancellation.check()?;
                return Ok(StageProgressV1::Checkpoint(bytes));
            }
            let a = session
                .restore_first_checkpoint(&checkpoints[0], budget)
                .map_err(|_| Error::Proof("Bootstrap A1 restore"))?;
            cancellation.check()?;
            if checkpoints.len() == 1 {
                let key = sources
                    .import_w(route, 0, originals, read)
                    .map_err(|_| Error::Proof("Bootstrap W0 original"))?;
                cancellation.check()?;
                let w = session
                    .wrapper(
                        &a,
                        &key,
                        Fq::random(OsRng),
                        &fold,
                        ProverRandomness::hedged(),
                        config,
                    )
                    .map_err(|_| Error::Proof("Bootstrap W0 prove"))?;
                let bytes = session
                    .encode_wrapper_checkpoint(&w, budget)
                    .map_err(|_| Error::Proof("Bootstrap W0 retain"))?;
                cancellation.check()?;
                return Ok(StageProgressV1::Checkpoint(bytes));
            }
            let w = session
                .restore_wrapper_checkpoint(&checkpoints[1], budget)
                .map_err(|_| Error::Proof("Bootstrap W0 restore"))?;
            cancellation.check()?;
            if checkpoints.len() == 2 {
                let key = sources
                    .import_a(route, 1, originals, read)
                    .map_err(|_| Error::Proof("Bootstrap A2 original"))?;
                cancellation.check()?;
                let salt = Fp::random(OsRng);
                let a = session
                    .terminal(&w, &key, salt, &fold, ProverRandomness::hedged(), config)
                    .map_err(|_| Error::Proof("Bootstrap A2 prove"))?;
                let bytes = session
                    .encode_terminal_checkpoint(&w, &a, salt, budget)
                    .map_err(|_| Error::Proof("Bootstrap A2 retain"))?;
                cancellation.check()?;
                return Ok(StageProgressV1::Checkpoint(bytes));
            }
            let a = session
                .restore_terminal_checkpoint(&w, &checkpoints[2], budget)
                .map_err(|_| Error::Proof("Bootstrap A2 restore"))?;
            complete(
                installed,
                sources,
                originals,
                read,
                owner,
                a.proof,
                a.instances,
                a.pallas,
                public,
                cancellation,
                budget,
            )
        }
        (QualifiedOperationOwnerV1::Load(p), Inputs::Load(i)) => run_staged!(
            source_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        (QualifiedOperationOwnerV1::Send(p), Inputs::Send(i)) => run_staged!(
            source_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        (QualifiedOperationOwnerV1::Receive(p), Inputs::Receive(i)) => run_staged!(
            key_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        (QualifiedOperationOwnerV1::Archive(p), Inputs::Archive(i)) => run_staged!(
            key_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        (QualifiedOperationOwnerV1::Consuming(p), Inputs::Consuming(i)) => run_staged!(
            source_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        (QualifiedOperationOwnerV1::Refresh(p), Inputs::Refresh(i)) => run_staged!(
            source_first,
            p,
            i,
            owner,
            public,
            checkpoints,
            cancellation,
            budget,
            installed,
            sources,
            route,
            originals,
            read
        ),
        _ => Err(Error::Proof(
            "native logical route and typed operation disagree",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_follow_all_native_checkpoint_schedules_and_reject_extra_or_even_counts() {
        for stages in [2usize, 4, 5, 10, 11] {
            let count = 2 * stages - 1;
            for completed in 0..count {
                assert_eq!(
                    next_key(completed, count).unwrap(),
                    if completed % 2 == 0 {
                        NextKey::A(completed / 2)
                    } else {
                        NextKey::W(completed / 2)
                    }
                );
            }
            assert_eq!(next_key(count, count).unwrap(), NextKey::Omega);
            assert!(next_key(count + 1, count).is_err());
        }
        for count in [0, 2, 4, 8, 20] {
            assert!(next_key(0, count).is_err());
        }
    }
}
