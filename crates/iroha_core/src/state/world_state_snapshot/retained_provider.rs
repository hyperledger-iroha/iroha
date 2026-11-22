//! Current provider proofs retain the exact original pre-tail World while certificate work runs.
use super::*;
use iroha_data_model::sorafs::capacity::ProviderId;

type CustodyRows<'a> = (&'a AccountId, Option<StreamTokenOriginals<'a>>);

pub(super) fn admission_rows<'a>(
    snapshot: &'a WorldStateSnapshotV1,
    world: &'a impl WorldReadOnly,
    provider: ProviderId,
    budget: &AllocationBudget,
) -> Result<ProviderAdmissionSnapshotOriginalsV1<'a>, WorldStateSnapshotError> {
    let (council_head, council_predecessor) = admission_originals(snapshot, world, None, budget)?;
    let (provider_head, provider_predecessor) =
        admission_originals(snapshot, world, Some(provider), budget)?;
    let owner = world
        .provider_owners()
        .get(&provider)
        .ok_or("World snapshot native provider owner is absent")?;
    require_target(
        snapshot,
        "world.provider_owners",
        WorldStateElementKindV1::Table,
        Some(hash_value(&provider)?),
        hash_value(owner)?,
    )?;
    Ok(ProviderAdmissionSnapshotOriginalsV1 {
        world: snapshot,
        council_head,
        council_predecessor,
        provider_head,
        provider_predecessor,
        owner,
        stream_token: stream_token_originals(snapshot, world, provider, budget)?,
    })
}

pub(super) fn custody_rows<'a>(
    snapshot: &WorldStateSnapshotV1,
    world: &'a impl WorldReadOnly,
    provider: ProviderId,
    budget: &AllocationBudget,
) -> Result<CustodyRows<'a>, WorldStateSnapshotError> {
    if provider.as_bytes() == &[0; 32]
        || crate::sumeragi::lanes::routing::committed_root_scope(world)
            != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
    {
        return Err("Custody projection requires a nonzero provider on the global root".into());
    }
    let owner = world
        .provider_owners()
        .get(&provider)
        .ok_or("World snapshot native provider owner is absent")?;
    require_target(
        snapshot,
        "world.provider_owners",
        WorldStateElementKindV1::Table,
        Some(hash_value(&provider)?),
        hash_value(owner)?,
    )?;
    let current = stream_token_originals(snapshot, world, provider, budget)?;
    if current.is_none() {
        use iroha_data_model::sorafs::stream_token_custody::history::{head_key, record_key};
        // Current typed absence cannot hide a value in the certified pre-tail cut.
        for key in [head_key(provider), record_key(provider, 1)] {
            let target = (
                "world.smart_contract_state",
                WorldStateElementKindV1::Table,
                Some(hash_value(&key)?),
            );
            if snapshot
                .entries
                .binary_search_by(|entry| {
                    (entry.field_id.as_str(), entry.kind, entry.key_hash).cmp(&target)
                })
                .is_ok()
            {
                return Err("Custody absence differs from the certified original cut".into());
            }
        }
    }

    Ok((owner, current))
}

/// Complete, physically acquired provider originals at a requested current native height.
/// The source view and allocation charges live until all proof work and rechecks finish.
pub struct NativeProviderSnapshotV1<'a> {
    snapshot: CapturedSnapshot,
    view: StateView<'a>,
    state: &'a State,
    provider: ProviderId,
    budget: &'a AllocationBudget,
}

#[derive(Clone, Copy)]
enum Rows<'a> {
    Admission(ProviderAdmissionSnapshotOriginalsV1<'a>),
    Custody(CustodyRows<'a>),
}

fn global(world: &impl WorldReadOnly) -> bool {
    crate::sumeragi::lanes::routing::committed_root_scope(world)
        == Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
}

impl State {
    /// Capture the exact currently requested Global World before certificate or encoding work.
    ///
    /// # Errors
    /// A changed height, private root, invalid provider, missing journal or finite allocation refusal.
    pub fn capture_native_provider_snapshot_v1<'a>(
        &'a self,
        height: u64,
        provider: ProviderId,
        budget: &'a AllocationBudget,
    ) -> Result<NativeProviderSnapshotV1<'a>, WorldStateSnapshotError> {
        if provider.as_bytes() == &[0; 32] || height < 2 {
            return Err("invalid native provider capture selector".into());
        }
        let (view, snapshot) = self.capture_original_snapshot(Some(height), None, budget)?;
        if !global(view.world()) {
            return Err("native provider capture requires the Global root".into());
        }
        Ok(NativeProviderSnapshotV1 {
            snapshot,
            view,
            state: self,
            provider,
            budget,
        })
    }
}

impl<'a> NativeProviderSnapshotV1<'a> {
    /// Immutable State source for independent certification of the requested height.
    pub fn view(&self) -> &StateView<'a> {
        &self.view
    }

    fn bind(&self, tip: &CommittedBlock) -> Result<(), WorldStateSnapshotError> {
        require_cut(&self.view, tip)?;
        if tip.commitment().execution.world_state_root
            != self.snapshot.snapshot.root().map_err(|e| e.to_string())?
        {
            return Err("retained provider World differs from certified root".into());
        }
        Ok(())
    }

    fn recheck(
        &self,
        tip: &CommittedBlock,
        originals: Rows<'_>,
        expiry: Option<u64>,
    ) -> Result<(), WorldStateSnapshotError> {
        let current = self.state.try_view_once()?;
        let origin_height = tip.height();
        let qualified_time = |view: &StateView<'_>| -> Result<(), WorldStateSnapshotError> {
            if let Some(expires) = expiry
                && view
                    .authenticated_query_ledger_time_ms()
                    .ok_or("missing authenticated provider expiry clock")?
                    / 1000
                    >= expires
            {
                return Err("retained provider authorization expired".into());
            }
            Ok(())
        };
        if current.publication_generation == self.view.publication_generation {
            require_cut(&current, tip)?;
            qualified_time(&current)?;
            return self.require_rows(originals, &self.snapshot.snapshot, current.world());
        }
        drop(current);
        // A new decision may carry independent writes. Read a complete current pre-tail
        // publication under the same request pool, then compare the actual selected originals.
        let (view, current) = self
            .state
            .capture_original_snapshot(None, None, self.budget)?;
        if view.native_world_snapshot_height_v1()? <= origin_height
            || !global(view.world())
            || view.network_id() != self.view.network_id()
            || view
                .block_hashes()
                .get(usize::try_from(origin_height - 1).map_err(|e| e.to_string())?)
                != Some(&tip.block_hash())
            || current.snapshot.schema_hash != self.snapshot.snapshot.schema_hash
        {
            return Err(
                "current provider authority does not extend the selected Global cut".into(),
            );
        }
        qualified_time(&view)?;
        self.require_rows(originals, &current.snapshot, view.world())
    }

    fn require_rows(
        &self,
        originals: Rows<'_>,
        snapshot: &WorldStateSnapshotV1,
        world: &impl WorldReadOnly,
    ) -> Result<(), WorldStateSnapshotError> {
        let matches = match originals {
            Rows::Admission(original) => {
                let other = admission_rows(snapshot, world, self.provider, self.budget)?;
                original.council_head == other.council_head
                    && original.council_predecessor == other.council_predecessor
                    && original.provider_head == other.provider_head
                    && original.provider_predecessor == other.provider_predecessor
                    && original.owner == other.owner
                    && original.stream_token == other.stream_token
            }
            Rows::Custody(original) => {
                original == custody_rows(snapshot, world, self.provider, self.budget)?
            }
        };
        if !matches {
            return Err("current provider authority differs from captured originals".into());
        }
        Ok(())
    }

    /// Publish only the original provider sources while council, admission, owner and custody remain current.
    ///
    /// # Errors
    /// The original Global root, selected current originals, expiry, finite budget or encoder differs.
    pub fn with_admission<T>(
        &self,
        tip: &CommittedBlock,
        expires_at_unix_secs: u64,
        consume: impl FnOnce(ProviderAdmissionSnapshotOriginalsV1<'_>) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        self.bind(tip)?;
        let rows = admission_rows(
            &self.snapshot.snapshot,
            self.view.world(),
            self.provider,
            self.budget,
        )?;
        self.recheck(tip, Rows::Admission(rows), Some(expires_at_unix_secs))?;
        let result = consume(rows)?;
        self.recheck(tip, Rows::Admission(rows), Some(expires_at_unix_secs))?;
        Ok(result)
    }

    /// Publish custody presence or proven absence independently of provider admission.
    ///
    /// # Errors
    /// The original Global root, selected current owner/control, finite budget or encoder differs.
    pub fn with_custody<T>(
        &self,
        tip: &CommittedBlock,
        consume: impl FnOnce(
            &WorldStateSnapshotV1,
            &AccountId,
            Option<(&Vec<u8>, &Vec<u8>)>,
        ) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        self.bind(tip)?;
        let rows = custody_rows(
            &self.snapshot.snapshot,
            self.view.world(),
            self.provider,
            self.budget,
        )?;
        self.recheck(tip, Rows::Custody(rows), None)?;
        let result = consume(&self.snapshot.snapshot, rows.0, rows.1)?;
        self.recheck(tip, Rows::Custody(rows), None)?;
        Ok(result)
    }
}
