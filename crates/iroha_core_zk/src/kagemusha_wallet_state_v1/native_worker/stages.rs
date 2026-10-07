//! One genuine native A/W task with complete ordered source restoration.

use ff::Field;

use super::*;

// The native families deliberately retain their typed source capabilities. This
// macro shares the scheduler only; every proof/restore dispatch remains static.
macro_rules! chain {
    ($worker:expr, $route:expr, $owner:expr, $inputs:expr, $checkpoints:expr,
     $originals:expr, $cancel:expr, $public:expr, $order:ident) => {{
        let owner = $owner;
        let session = proof(owner.prepare($inputs, $worker.budget))?;
        let layouts = proof(owner.checkpoint_layouts())?;
        let originals = $originals;
        let checkpoints = $checkpoints;
        if checkpoints.len() > layouts.len() {
            return Err(Error::Proof("extra native checkpoint"));
        }
        let mut a = None;
        let mut w = None;
        for (position, original) in checkpoints.iter().enumerate() {
            $cancel.check()?;
            if original.len() != layouts[position].payload_bytes() {
                return Err(Error::Proof("native checkpoint length"));
            }
            if position == 0 {
                a = Some(proof(
                    session.restore_first_checkpoint(original, $worker.budget),
                )?);
            } else if position % 2 == 1 {
                w = Some(proof(session.restore_wrapper_checkpoint(
                    a.as_ref().ok_or(Error::Proof("native A source"))?,
                    original,
                    $worker.budget,
                ))?);
            } else {
                a = Some(proof(session.restore_a_checkpoint(
                    w.as_ref().ok_or(Error::Proof("native W source"))?,
                    original,
                    $worker.budget,
                ))?);
            }
        }
        $cancel.check()?;
        let position = checkpoints.len();
        if position == layouts.len() {
            let terminal = proof(session.terminal(
                a.as_ref().ok_or(Error::Proof("native terminal source"))?,
                $worker.budget,
            ))?;
            NativeStageProgressV1::Terminal(omega::Input {
                key: proof($worker.sources.route($route))?
                    .terminal()
                    .key()
                    .clone(),
                proof: terminal.proof,
                frame: terminal
                    .instances
                    .try_into()
                    .map_err(|_| Error::Proof("terminal frame"))?,
                public: $public,
                pallas: terminal.pallas,
            })
        } else {
            let fold = $worker.fold_config();
            let config = $worker.prover_config();
            let encoded = if position == 0 {
                let key = $worker
                    .sources
                    .import_a($route, 0, originals, $worker.read)
                    .map_err(artifact)?;
                $cancel.check()?;
                let source = proof(session.first(
                    &key,
                    Fp::random(rand_core_06::OsRng),
                    &fold,
                    ProverRandomness::os(),
                    config,
                ))?;
                proof(session.encode_a_checkpoint(&source, $worker.budget))?
            } else if position % 2 == 1 {
                let key = $worker
                    .sources
                    .import_w($route, position / 2, originals, $worker.read)
                    .map_err(artifact)?;
                $cancel.check()?;
                let source = a.as_ref().ok_or(Error::Proof("native A source"))?;
                let next = proof(call_wrapper!($order, session, source, key, fold, config))?;
                proof(session.encode_wrapper_checkpoint(&next, $worker.budget))?
            } else {
                let key = $worker
                    .sources
                    .import_a($route, position / 2, originals, $worker.read)
                    .map_err(artifact)?;
                $cancel.check()?;
                let source = w.as_ref().ok_or(Error::Proof("native W source"))?;
                let next = proof(call_advance!($order, session, source, key, fold, config))?;
                proof(session.encode_a_checkpoint(&next, $worker.budget))?
            };
            if encoded.len() != layouts[position].payload_bytes() {
                return Err(Error::Proof("native output length"));
            }
            $cancel.check()?;
            NativeStageProgressV1::Checkpoint(encoded)
        }
    }};
}
macro_rules! call_wrapper {
    (source_first,$session:ident,$source:ident,$key:ident,$fold:ident,$config:ident) => {
        $session.wrapper(
            $source,
            &$key,
            Fq::random(rand_core_06::OsRng),
            &$fold,
            ProverRandomness::os(),
            $config,
        )
    };
    (key_first,$session:ident,$source:ident,$key:ident,$fold:ident,$config:ident) => {
        $session.wrapper(
            &$key,
            $source,
            Fq::random(rand_core_06::OsRng),
            &$fold,
            ProverRandomness::os(),
            $config,
        )
    };
}
macro_rules! call_advance {
    (source_first,$session:ident,$source:ident,$key:ident,$fold:ident,$config:ident) => {
        $session.advance(
            $source,
            &$key,
            Fp::random(rand_core_06::OsRng),
            &$fold,
            ProverRandomness::os(),
            $config,
        )
    };
    (key_first,$session:ident,$source:ident,$key:ident,$fold:ident,$config:ident) => {
        $session.advance(
            &$key,
            $source,
            Fp::random(rand_core_06::OsRng),
            &$fold,
            ProverRandomness::os(),
            $config,
        )
    };
}

impl NativeFoldWorkerV1 {
    /// Restore all prior A/W sources and produce exactly one next proof. Once the
    /// terminal checkpoint exists, return its verified source to the separate Omega task.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stage(
        &self,
        route: OperationRoute,
        inputs: OperationInputsV1,
        public: [Fp; 18],
        checkpoints: &[Vec<u8>],
        originals: &mut dyn OriginalSourceV1,
        cancellation: &Cancellation,
    ) -> Result<NativeStageProgressV1, Error> {
        cancellation.check()?;
        let selected = proof(self.sources.route(route))?;
        Ok(match (selected.owner(), inputs) {
            (QualifiedOperationOwnerV1::Bootstrap(owner), OperationInputsV1::Bootstrap(input)) => {
                let owner = owner.prover();
                let layouts = proof(owner.checkpoint_layouts())?;
                if checkpoints.len() > layouts.len() {
                    return Err(Error::Proof("extra Bootstrap checkpoint"));
                }
                let session = proof(owner.prepare(input, self.budget))?;
                let mut first = None;
                let mut wrapper = None;
                let mut terminal = None;
                for (position, original) in checkpoints.iter().enumerate() {
                    cancellation.check()?;
                    if original.len() != layouts[position].payload_bytes() {
                        return Err(Error::Proof("Bootstrap checkpoint length"));
                    }
                    match position {
                        0 => {
                            first = Some(proof(
                                session.restore_first_checkpoint(original, self.budget),
                            )?)
                        }
                        1 => {
                            wrapper = Some(proof(
                                session.restore_wrapper_checkpoint(original, self.budget),
                            )?)
                        }
                        2 => {
                            terminal = Some(proof(session.restore_terminal_checkpoint(
                                wrapper.as_ref().ok_or(Error::Proof("Bootstrap W source"))?,
                                original,
                                self.budget,
                            ))?)
                        }
                        _ => return Err(Error::Proof("Bootstrap stage")),
                    }
                }
                cancellation.check()?;
                if let Some(terminal) = terminal {
                    NativeStageProgressV1::Terminal(omega::Input {
                        key: selected.terminal().key().clone(),
                        proof: terminal.proof,
                        frame: terminal
                            .instances
                            .try_into()
                            .map_err(|_| Error::Proof("Bootstrap terminal frame"))?,
                        public,
                        pallas: terminal.pallas,
                    })
                } else {
                    let position = checkpoints.len();
                    let fold = self.fold_config();
                    let config = self.prover_config();
                    let encoded = match position {
                        0 => {
                            let key = self
                                .sources
                                .import_a(route, 0, originals, self.read)
                                .map_err(artifact)?;
                            cancellation.check()?;
                            let source =
                                proof(session.first(&key, ProverRandomness::os(), config))?;
                            proof(session.encode_first_checkpoint(&source, self.budget))?
                        }
                        1 => {
                            let key = self
                                .sources
                                .import_w(route, 0, originals, self.read)
                                .map_err(artifact)?;
                            cancellation.check()?;
                            let source = proof(session.wrapper(
                                first.as_ref().ok_or(Error::Proof("Bootstrap A source"))?,
                                &key,
                                Fq::random(rand_core_06::OsRng),
                                &fold,
                                ProverRandomness::os(),
                                config,
                            ))?;
                            proof(session.encode_wrapper_checkpoint(&source, self.budget))?
                        }
                        2 => {
                            let key = self
                                .sources
                                .import_a(route, 1, originals, self.read)
                                .map_err(artifact)?;
                            cancellation.check()?;
                            let prior =
                                wrapper.as_ref().ok_or(Error::Proof("Bootstrap W source"))?;
                            let salt = Fp::random(rand_core_06::OsRng);
                            let source = proof(session.terminal(
                                prior,
                                &key,
                                salt,
                                &fold,
                                ProverRandomness::os(),
                                config,
                            ))?;
                            proof(session.encode_terminal_checkpoint(
                                prior,
                                &source,
                                salt,
                                self.budget,
                            ))?
                        }
                        _ => return Err(Error::Proof("Bootstrap stage")),
                    };
                    if encoded.len() != layouts[position].payload_bytes() {
                        return Err(Error::Proof("Bootstrap output length"));
                    }
                    cancellation.check()?;
                    NativeStageProgressV1::Checkpoint(encoded)
                }
            }
            (QualifiedOperationOwnerV1::Load(owner), OperationInputsV1::Load(input)) => chain!(
                self,
                route,
                owner,
                input,
                checkpoints,
                originals,
                cancellation,
                public,
                source_first
            ),
            (QualifiedOperationOwnerV1::Send(owner), OperationInputsV1::Send(input)) => chain!(
                self,
                route,
                owner,
                input,
                checkpoints,
                originals,
                cancellation,
                public,
                source_first
            ),
            (QualifiedOperationOwnerV1::Receive(owner), OperationInputsV1::Receive(input)) => {
                chain!(
                    self,
                    route,
                    owner,
                    input,
                    checkpoints,
                    originals,
                    cancellation,
                    public,
                    key_first
                )
            }
            (QualifiedOperationOwnerV1::Archive(owner), OperationInputsV1::Archive(input)) => {
                chain!(
                    self,
                    route,
                    owner,
                    input,
                    checkpoints,
                    originals,
                    cancellation,
                    public,
                    key_first
                )
            }
            (QualifiedOperationOwnerV1::Consuming(owner), OperationInputsV1::Consuming(input)) => {
                chain!(
                    self,
                    route,
                    owner,
                    input,
                    checkpoints,
                    originals,
                    cancellation,
                    public,
                    source_first
                )
            }
            (QualifiedOperationOwnerV1::Refresh(owner), OperationInputsV1::Refresh(input)) => {
                chain!(
                    self,
                    route,
                    owner,
                    input,
                    checkpoints,
                    originals,
                    cancellation,
                    public,
                    source_first
                )
            }
            _ => return Err(Error::Proof("native route/input mismatch")),
        })
    }
}
