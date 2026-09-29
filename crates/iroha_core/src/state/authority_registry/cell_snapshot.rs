//! Bounded, non-authorizing capture of five native-Norito State cells.
//!
//! This is one traversal increment, not a complete State commitment. It issues
//! no aggregate root, witness, finalized anchor, or admission authority. The
//! caller must retry when State publication overlaps the observed generation.
//! Direct MV writes outside State publication do not advance that counter, so
//! this diagnostic slice cannot certify one cross-cell cut or authorize use.

use super::{Canonical, Role, STATE_FIELDS, Schema, V1_LAYOUT};
use crate::state::{State, is_stable_state_view_generation};
use iroha_crypto::Hash;
use norito::{NoritoSchema, codec::Encode};
use std::io::{self, Write};

const CELL_DOMAIN: &[u8] = b"iroha:state-cell-slice:bare-v1\0";

/// Maximum emitted bytes for each cell and for this whole capture.
#[derive(Clone, Copy)]
pub(in crate::state) struct CellSliceLimits {
    /// One cell's maximum bare Norito payload length.
    pub max_cell_payload_bytes: usize,
    /// Aggregate maximum bare Norito payload length across five cells.
    pub max_total_payload_bytes: usize,
}

/// Five named diagnostic digests observed under an unchanged State publication counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::state) struct StateCellDigestSliceV1 {
    chain_id: Hash,
    network_id: Hash,
    commit_topology: Hash,
    prev_commit_topology: Hash,
    lane_consensus_contexts: Hash,
}

struct BoundedPayloadWriter<'a> {
    inner: &'a mut dyn Write,
    remaining: usize,
    exceeded: &'a mut bool,
}

impl Write for BoundedPayloadWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            *self.exceeded = true;
            return Err(io::Error::other(
                "State cell payload exceeds the capture limit",
            ));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn hash_cell<T: Encode + NoritoSchema>(
    id: &'static str,
    value: &T,
    limits: CellSliceLimits,
    remaining: &mut usize,
) -> Result<Hash, String> {
    let field = STATE_FIELDS
        .iter()
        .find(|field| field.id == id)
        .ok_or_else(|| format!("unclassified State cell {id}"))?;
    let Role::Canonical(Canonical::Cell(Schema::Norito {
        nominal_name,
        layout,
    })) = field.role
    else {
        return Err(format!("State cell {id} has no native Norito schema"));
    };
    if layout != V1_LAYOUT {
        return Err(format!("State cell {id} uses a non-V1 layout"));
    }
    let declared = nominal_name();
    if declared != T::nominal_name() {
        return Err(format!("State cell {id} has a mismatched nominal type"));
    }
    let mut exceeded = false;
    let emitted_limit = limits.max_cell_payload_bytes.min(*remaining);
    let mut written_payload = 0_usize;
    let digest = Hash::new_from_writer(|writer| {
        writer.write_all(CELL_DOMAIN)?;
        writer.write_all(
            &u64::try_from(id.len())
                .map_err(io::Error::other)?
                .to_le_bytes(),
        )?;
        writer.write_all(id.as_bytes())?;
        writer.write_all(
            &u64::try_from(declared.len())
                .map_err(io::Error::other)?
                .to_le_bytes(),
        )?;
        writer.write_all(declared.as_bytes())?;
        writer.write_all(&[layout.major, layout.minor, layout.flags])?;
        written_payload = {
            let mut bounded = BoundedPayloadWriter {
                inner: writer,
                remaining: emitted_limit,
                exceeded: &mut exceeded,
            };
            norito::codec::encode_adaptive_into(value, &mut bounded).map_err(io::Error::other)?
        };
        writer.write_all(
            &u64::try_from(written_payload)
                .map_err(io::Error::other)?
                .to_le_bytes(),
        )
    });
    if exceeded {
        return Err(format!("State cell {id} exceeds the capture limit"));
    }
    let digest = digest.map_err(|error| format!("State cell {id} encoding failed: {error}"))?;
    *remaining -= written_payload;
    Ok(digest)
}

/// Capture five registry-declared cells while no tracked State publication overlaps.
///
/// `None` means a coordinated State publisher won the generation race. `Some`
/// does not certify one atomic cut when a direct MV writer bypasses State
/// publication. Errors leave no partially usable digest slice. The bounded
/// writer streams each bare Norito payload into the hash.
pub(in crate::state) fn capture_once(
    state: &State,
    limits: CellSliceLimits,
) -> Result<Option<StateCellDigestSliceV1>, String> {
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Ok(None);
    }
    let commit_topology = state.commit_topology.view();
    let prev_commit_topology = state.prev_commit_topology.view();
    let lane_consensus_contexts = state.lane_consensus_contexts.view();
    let mut remaining = limits.max_total_payload_bytes;
    let slice = StateCellDigestSliceV1 {
        chain_id: hash_cell("state.chain_id", &state.chain_id, limits, &mut remaining)?,
        network_id: hash_cell(
            "state.network_id",
            &state.network_id,
            limits,
            &mut remaining,
        )?,
        commit_topology: hash_cell(
            "state.commit_topology",
            commit_topology.get(),
            limits,
            &mut remaining,
        )?,
        prev_commit_topology: hash_cell(
            "state.prev_commit_topology",
            prev_commit_topology.get(),
            limits,
            &mut remaining,
        )?,
        lane_consensus_contexts: hash_cell(
            "state.lane_consensus_contexts",
            lane_consensus_contexts.get(),
            limits,
            &mut remaining,
        )?,
    };
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Ok(None);
    }
    Ok(Some(slice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::peer::PeerId;

    fn state() -> State {
        State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn limits() -> CellSliceLimits {
        CellSliceLimits {
            max_cell_payload_bytes: 1024 * 1024,
            max_total_payload_bytes: 5 * 1024 * 1024,
        }
    }

    #[test]
    fn captures_named_cells_and_detects_topology_mutation() {
        let state = state();
        let generation = state.state_view_generation();
        let before = capture_once(&state, limits()).unwrap().unwrap();
        assert_eq!(before, capture_once(&state, limits()).unwrap().unwrap());
        let peer = PeerId::new(
            KeyPair::from_seed(b"state-cell-slice-peer".to_vec(), Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let mut topology = state.commit_topology.block();
        topology.push(peer);
        topology.commit();
        assert_eq!(
            state.state_view_generation(),
            generation,
            "direct MV publication is not covered by the State counter"
        );
        let after = capture_once(&state, limits()).unwrap().unwrap();
        assert_ne!(before.commit_topology, after.commit_topology);
        assert_eq!(before.chain_id, after.chain_id);
        assert_eq!(before.network_id, after.network_id);
        assert_eq!(before.prev_commit_topology, after.prev_commit_topology);
        assert_eq!(
            before.lane_consensus_contexts,
            after.lane_consensus_contexts
        );
    }

    #[test]
    fn capture_refuses_nominal_type_and_payload_bounds() {
        let state = state();
        let mut remaining = 1024;
        assert!(
            hash_cell("state.chain_id", &0_u64, limits(), &mut remaining)
                .unwrap_err()
                .contains("mismatched nominal type")
        );
        assert_eq!(remaining, 1024);
        assert!(
            capture_once(
                &state,
                CellSliceLimits {
                    max_cell_payload_bytes: 0,
                    max_total_payload_bytes: 0,
                },
            )
            .unwrap_err()
            .contains("capture limit")
        );
    }

    #[test]
    fn active_state_publication_returns_retry_without_partial_slice() {
        let state = state();
        let mut publication = state.state_view_publication();
        let guard = publication.begin();
        assert!(capture_once(&state, limits()).unwrap().is_none());
        drop(guard);
        drop(publication);
        assert!(capture_once(&state, limits()).unwrap().is_some());
    }
}
