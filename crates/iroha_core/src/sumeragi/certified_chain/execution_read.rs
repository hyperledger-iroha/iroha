//! Execution-read authority from the native quorum or the actual genesis successor anchor.
use super::*;

/// An original block whose execution result, not only its header, has been authenticated.
///
/// This receipt is never decoded or publicly constructed. Ordinary blocks require their native
/// quorum; genesis requires its verified H2 successor because genesis signatures do not bind R.
#[derive(Debug)]
pub struct AuthenticatedExecutionBlock {
    committed: CommittedBlock,
}
impl AuthenticatedExecutionBlock {
    /// Original authenticated block graph, including the exact node-local certificate.
    #[must_use]
    pub fn block(&self) -> &Arc<SignedBlock> {
        self.committed.block()
    }

    /// The authenticated execution result and exact source identity.
    #[must_use]
    pub fn committed(&self) -> &CommittedBlock {
        &self.committed
    }
}
impl CertifiedBlock {
    /// Retain execution-read authority from the native quorum without cloning the block graph.
    ///
    /// # Errors
    /// A genesis body signature alone cannot authenticate its subsequently added execution R.
    pub fn into_authenticated_execution(
        self,
    ) -> Result<AuthenticatedExecutionBlock, ChainReadError> {
        if self.verification != QcVerification::Verified {
            return Err(ChainReadError::Malformed {
                height: self.height(),
                reason: "genesis execution requires its verified height-two successor".into(),
            });
        }
        Ok(AuthenticatedExecutionBlock {
            committed: self.committed,
        })
    }
}
impl GenesisExecutionAnchor {
    /// Consume the actual H2 anchor while retaining the exact authenticated genesis graph.
    #[must_use]
    pub fn into_authenticated_execution(self) -> AuthenticatedExecutionBlock {
        AuthenticatedExecutionBlock {
            committed: self.committed,
        }
    }
}
impl<V: StateReadOnly + ?Sized> CertifiedChain<'_, V> {
    /// Authenticate the original execution at one committed height, including H1's H2 anchor.
    ///
    /// # Errors
    /// Missing or substituted history, invalid native certificates, or unavailable H2 for H1.
    pub fn authenticated_execution(
        &self,
        height: u64,
    ) -> Result<AuthenticatedExecutionBlock, ChainReadError> {
        if height != GENESIS_HEIGHT {
            return self.certified(height)?.into_authenticated_execution();
        }
        let mut prefix = CertifiedPrefix::new(
            self.source.chain_id(),
            *self.source.network_id(),
            self.source.block(GENESIS_HEIGHT)?,
        )?;
        let (_, genesis) = prefix
            .push(self.source.block(GENESIS_HEIGHT + 1)?)?
            .into_parts();
        genesis
            .map(GenesisExecutionAnchor::into_authenticated_execution)
            .ok_or_else(|| ChainReadError::Malformed {
                height,
                reason: "native successor did not authenticate genesis execution".into(),
            })
    }
}

/// Explicit finite ceilings for an off-chain native prefix read.
///
/// Every source frame, including signed genesis and H2 for a genesis request, counts. These
/// logical byte/work bounds are not a reservation of decoded resident graphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeExecutionReadLimits {
    /// Exact requested target length admitted by its consumer before source verification.
    /// A changed occupied target is refused before its body is read, even below the frame cap.
    pub admitted_target_wire_bytes: u64,
    /// Maximum number of source blocks authenticated, counted before any body I/O.
    pub max_source_blocks: u64,
    /// Aggregate canonical frame bytes across all source blocks.
    pub max_source_wire_bytes: u64,
    /// Maximum single source frame, also capped by Kura's native full-frame bound.
    pub max_frame_wire_bytes: u64,
}

/// Finite source allowance which was exhausted before reading the refused body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeExecutionReadResource {
    /// Number of source blocks.
    SourceBlocks,
    /// Aggregate source frame bytes.
    SourceWireBytes,
    /// One frame's wire length.
    FrameWireBytes,
}

/// Failure to acquire a fully authenticated native execution under explicit source limits.
#[derive(Debug, thiserror::Error)]
pub enum NativeExecutionReadError {
    /// A source limit was refused before the corresponding body read or decode.
    #[error("native execution source {resource:?} requires {actual}, limit {limit}")]
    Capacity {
        /// Refused finite resource.
        resource: NativeExecutionReadResource,
        /// Required quantity, saturated on arithmetic overflow.
        actual: u64,
        /// Caller-supplied allowance.
        limit: u64,
    },
    /// Bounded metadata, body I/O or exact canonical decoding failed.
    #[error("native execution storage at height {height}: {reason}")]
    Storage {
        /// Exact source height, which may precede the requested execution.
        height: u64,
        /// Underlying storage or decoder refusal.
        reason: String,
    },
    /// The actual native prefix, including the genesis successor anchor, did not authenticate.
    #[error(transparent)]
    Chain(#[from] ChainReadError),
}

/// Original authenticated graph and its exact canonical source bytes.
#[derive(Debug)]
pub struct NativeExecutionRead {
    /// Execution authority from the original native quorum or real H2 anchor.
    pub authority: AuthenticatedExecutionBlock,
    /// Exact bytes decoded once and used by that verification.
    pub wire: Vec<u8>,
    /// Number of source frames consumed by the verifier.
    pub source_blocks: u64,
    /// Aggregate canonical source frame bytes admitted before reading them.
    pub source_wire_bytes: u64,
}

fn admit_source(
    resource: NativeExecutionReadResource,
    actual: u64,
    limit: u64,
) -> Result<(), NativeExecutionReadError> {
    if actual > limit {
        return Err(NativeExecutionReadError::Capacity {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

/// Verify the actual native prefix from a borrowed immutable hash cut under explicit limits.
///
/// This is an off-chain proof/read boundary. Deterministic instruction execution must not
/// depend on local certificate bytes. It holds no World view or all-history frame vector,
/// does not consult the old finality sidecars, and never replaces missing bytes from a cache.
/// Every source length is admitted before body I/O and canonical decode. A genesis request
/// must include the actual H2 successor; a signed genesis body alone cannot authenticate Rg.
///
/// # Errors
/// Zero/insufficient allowances, unavailable or substituted history, invalid canonical frames
/// or native certificates. No partial execution authority escapes on failure.
pub fn read_authenticated_execution(
    kura: &Kura,
    chain_id: &ChainId,
    network: NetworkId,
    hashes: &dyn crate::state::BlockHashRead,
    height: u64,
    limits: NativeExecutionReadLimits,
) -> Result<NativeExecutionRead, NativeExecutionReadError> {
    if height == 0 {
        return Err(ChainReadError::NotCommitted { height }.into());
    }
    let end = height.max(GENESIS_HEIGHT + 1);
    admit_source(
        NativeExecutionReadResource::SourceBlocks,
        end,
        limits.max_source_blocks,
    )?;
    if usize::try_from(end)
        .ok()
        .is_none_or(|end| end > hashes.hash_count())
    {
        return Err(ChainReadError::NotCommitted { height: end }.into());
    }
    let mut total = 0_u64;
    let mut prefix = None;
    let mut retained_wire = None;
    for current in GENESIS_HEIGHT..=end {
        let expected = *hashes
            .hash_at(
                usize::try_from(current - 1)
                    .map_err(|_| ChainReadError::NotCommitted { height: current })?,
            )
            .ok_or(ChainReadError::NotCommitted { height: current })?;
        let storage = |error: crate::kura::Error| NativeExecutionReadError::Storage {
            height: current,
            reason: error.to_string(),
        };
        let source = kura
            .native_frame_read(current, expected)
            .map_err(storage)?
            .ok_or(ChainReadError::NotInView { height: current })?;
        let length = source.wire_len();
        if current == height && length != limits.admitted_target_wire_bytes {
            return Err(NativeExecutionReadError::Storage {
                height: current,
                reason: "native target length changed after consumer admission".into(),
            });
        }
        admit_source(
            NativeExecutionReadResource::FrameWireBytes,
            length,
            limits
                .max_frame_wire_bytes
                .min(crate::kura::STRICT_INIT_MAX_BLOCK_BYTES),
        )?;
        let next = total
            .checked_add(length)
            .ok_or(NativeExecutionReadError::Capacity {
                resource: NativeExecutionReadResource::SourceWireBytes,
                actual: u64::MAX,
                limit: limits.max_source_wire_bytes,
            })?;
        admit_source(
            NativeExecutionReadResource::SourceWireBytes,
            next,
            limits.max_source_wire_bytes,
        )?;
        let wire = source
            .read(length)
            .map_err(storage)?
            .ok_or(ChainReadError::NotInView { height: current })?;
        let block =
            iroha_data_model::block::decode_framed_signed_block(&wire).map_err(|error| {
                NativeExecutionReadError::Storage {
                    height: current,
                    reason: error.to_string(),
                }
            })?;
        if block.header().height().get() != current || block.hash() != expected {
            return Err(ChainReadError::NotInView { height: current }.into());
        }
        let identity =
            block
                .canonical_wire_identity()
                .map_err(|error| NativeExecutionReadError::Storage {
                    height: current,
                    reason: error.to_string(),
                })?;
        if identity != (length, Hash::new(&wire)) {
            return Err(ChainReadError::Malformed {
                height: current,
                reason: "stored native execution is not the exact canonical frame".into(),
            }
            .into());
        }
        total = next;
        let block = Arc::new(block);
        if current == GENESIS_HEIGHT {
            prefix = Some(CertifiedPrefix::new(chain_id, network, block)?);
            if height == GENESIS_HEIGHT {
                retained_wire = Some(wire);
            }
            continue;
        }
        let (certified, genesis) = prefix
            .as_mut()
            .expect("genesis verified first")
            .push(block)?
            .into_parts();
        if current == end {
            let (authority, wire) = if height == GENESIS_HEIGHT {
                (
                    genesis
                        .ok_or_else(|| ChainReadError::Malformed {
                            height,
                            reason: "H2 did not supply the genesis execution anchor".into(),
                        })?
                        .into_authenticated_execution(),
                    retained_wire.take().expect("original genesis wire"),
                )
            } else {
                (certified.into_authenticated_execution()?, wire)
            };
            // TODO: physically admit decoder/authority graphs and retained wire from the query
            // owner's original pool. Logical source ceilings are not that reservation.
            return Ok(NativeExecutionRead {
                authority,
                wire,
                source_blocks: end,
                source_wire_bytes: total,
            });
        }
    }
    unreachable!("the admitted prefix always includes H2")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };

    #[test]
    fn signed_genesis_cannot_supply_execution_authority_without_h2() {
        let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let view = chain.state().view();
        let source = CertifiedChain::new(&view).unwrap();
        assert!(
            source
                .certified(1)
                .unwrap()
                .into_authenticated_execution()
                .is_err()
        );
        assert!(source.authenticated_execution(1).is_err());
    }

    #[test]
    fn genuine_successor_authenticates_both_original_execution_graphs() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        assert_independent_execution_reads(&chain);
        assert_consumed_execution_graphs(&chain);
    }

    fn assert_independent_execution_reads(chain: &CertifiedTestChain) {
        let view = chain.state().view();
        let source = CertifiedChain::new(&view).unwrap();
        for height in 1..=2 {
            let authenticated = source.authenticated_execution(height).unwrap();
            assert_eq!(authenticated.committed().height(), height);
            let original = source.committed(height).unwrap();
            // Independent reads may decode separate owned graphs. Their exact
            // canonical carrier, header identity and execution result must agree.
            assert_eq!(
                authenticated.block().encode_wire().unwrap(),
                original.block().encode_wire().unwrap(),
                "independent source reads retain the same exact certified frame"
            );
            assert_eq!(
                authenticated.committed().block_hash(),
                original.block_hash()
            );
            assert_eq!(authenticated.committed().core_hash(), original.core_hash());
            assert_eq!(authenticated.committed().result(), original.result());
        }
    }

    fn assert_consumed_execution_graphs(chain: &CertifiedTestChain) {
        let view = chain.state().view();
        let source = CertifiedChain::new(&view).unwrap();
        // Consuming each verification receipt must retain its original graph. Separate
        // physical reads above need not share one allocation or a process-local cache.
        let original_genesis = Arc::clone(chain.committed(GENESIS_HEIGHT).block());
        let original_successor = Arc::clone(chain.committed(GENESIS_HEIGHT + 1).block());
        let mut prefix = CertifiedPrefix::new(
            view.chain_id(),
            *view.network_id(),
            Arc::clone(&original_genesis),
        )
        .unwrap();
        let (certified, genesis) = prefix
            .push(Arc::clone(&original_successor))
            .unwrap()
            .into_parts();
        let genesis = genesis.expect("actual H2 authenticates genesis execution");
        let authenticated_genesis = genesis.into_authenticated_execution();
        assert!(Arc::ptr_eq(
            authenticated_genesis.block(),
            &original_genesis
        ));
        let authenticated_successor = certified.into_authenticated_execution().unwrap();
        assert!(Arc::ptr_eq(
            authenticated_successor.block(),
            &original_successor
        ));
        for (height, authenticated) in [(1, &authenticated_genesis), (2, &authenticated_successor)]
        {
            assert_eq!(authenticated.committed().height(), height);
            assert_eq!(
                authenticated.block().encode_wire().unwrap(),
                source
                    .committed(height)
                    .unwrap()
                    .block()
                    .encode_wire()
                    .unwrap()
            );
        }
    }
    #[test]
    fn bounded_native_source_authenticates_original_h1_and_h2_frames() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let frames = (1..=2)
            .map(|height| chain.committed(height).block().encode_wire().unwrap())
            .collect::<Vec<_>>();
        let total = frames.iter().map(|wire| wire.len() as u64).sum::<u64>();
        let limits = NativeExecutionReadLimits {
            admitted_target_wire_bytes: frames[0].len() as u64,
            max_source_blocks: 2,
            max_source_wire_bytes: total,
            max_frame_wire_bytes: frames.iter().map(|wire| wire.len() as u64).max().unwrap(),
        };
        for height in 1..=2 {
            let read = read_authenticated_execution(
                chain.kura(),
                view.chain_id(),
                *view.network_id(),
                view.block_hashes(),
                height,
                NativeExecutionReadLimits {
                    admitted_target_wire_bytes: frames[(height - 1) as usize].len() as u64,
                    ..limits
                },
            )
            .unwrap();
            assert_eq!(read.wire, frames[(height - 1) as usize]);
            assert_eq!(read.authority.committed().height(), height);
            assert_eq!(read.source_blocks, 2);
            assert_eq!(read.source_wire_bytes, total);
        }
        for (limits, resource) in [
            (
                NativeExecutionReadLimits {
                    max_source_blocks: 1,
                    ..limits
                },
                NativeExecutionReadResource::SourceBlocks,
            ),
            (
                NativeExecutionReadLimits {
                    max_source_wire_bytes: total - 1,
                    ..limits
                },
                NativeExecutionReadResource::SourceWireBytes,
            ),
            (
                NativeExecutionReadLimits {
                    max_frame_wire_bytes: 0,
                    ..limits
                },
                NativeExecutionReadResource::FrameWireBytes,
            ),
        ] {
            assert!(
                matches!(read_authenticated_execution(chain.kura(), view.chain_id(), *view.network_id(), view.block_hashes(), 1, limits),
                Err(NativeExecutionReadError::Capacity { resource: found, .. }) if found == resource)
            );
        }
    }

    #[test]
    fn bounded_source_refuses_foreign_cut_and_genesis_without_actual_h2() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let limits = NativeExecutionReadLimits {
            admitted_target_wire_bytes: 0,
            max_source_blocks: 2,
            max_source_wire_bytes: 8 * 1024 * 1024,
            max_frame_wire_bytes: 8 * 1024 * 1024,
        };
        {
            let view = chain.state().view();
            assert!(matches!(
                read_authenticated_execution(
                    chain.kura(),
                    view.chain_id(),
                    *view.network_id(),
                    view.block_hashes(),
                    1,
                    limits
                ),
                Err(NativeExecutionReadError::Chain(
                    ChainReadError::NotCommitted { height: 2 }
                ))
            ));
        }
        chain.commit(Vec::new());
        let view = chain.state().view();
        let target_wire = chain
            .kura()
            .get_block(NonZeroUsize::new(2).unwrap())
            .unwrap()
            .encode_wire()
            .unwrap();
        let limits = NativeExecutionReadLimits {
            admitted_target_wire_bytes: target_wire.len() as u64,
            ..limits
        };
        let mut hashes = view.block_hashes().iter().copied().collect::<Vec<_>>();
        hashes[1] = HashOf::from_untyped_unchecked(Hash::new(b"foreign committed cut"));
        assert!(matches!(
            read_authenticated_execution(
                chain.kura(),
                view.chain_id(),
                *view.network_id(),
                &hashes,
                2,
                limits
            ),
            Err(NativeExecutionReadError::Storage { height: 2, .. })
        ));
        assert!(matches!(
            read_authenticated_execution(
                chain.kura(),
                &ChainId::from("foreign-chain"),
                *view.network_id(),
                view.block_hashes(),
                2,
                limits
            ),
            Err(NativeExecutionReadError::Chain(
                ChainReadError::WrongInstance { height: 2 }
            ))
        ));
    }
    #[test]
    fn target_length_substitution_is_refused_under_an_otherwise_sufficient_budget() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let original = chain.committed(2).block().encode_wire().unwrap();
        for admitted in [0, original.len() as u64 - 1, original.len() as u64 + 1] {
            let limits = NativeExecutionReadLimits {
                admitted_target_wire_bytes: admitted,
                max_source_blocks: 2,
                max_source_wire_bytes: 8 * 1024 * 1024,
                max_frame_wire_bytes: 8 * 1024 * 1024,
            };
            assert!(matches!(
                read_authenticated_execution(
                    chain.kura(),
                    view.chain_id(),
                    *view.network_id(),
                    view.block_hashes(),
                    2,
                    limits
                ),
                Err(NativeExecutionReadError::Storage { height: 2, .. })
            ));
        }
    }
}
