//! Original complete-World authority for the read-only retirement source census.

use super::{State, StateQueryView, StateReadOnly, WorldReadOnly, WorldStateSnapshotError};
use crate::{
    execution_attempt::ExecutionAttemptError,
    sumeragi::certified_chain::{CertifiedChain, ChainReadError},
};
mod keyed;

/// Exact existing World value hash while retaining the encoder's original refusal.
pub(in crate::state) fn original_hash<T: norito::codec::Encode>(
    value: &T,
) -> Result<iroha_crypto::Hash, WorldStateSnapshotError> {
    let mut encoding = Ok(());
    let hash = iroha_crypto::Hash::new_from_writer(|mut writer| {
        writer.write_all(b"iroha:world-net-delta:value:bare-v1\0")?;
        match norito::core::classify_decode_attempt(|| {
            norito::codec::encode_adaptive_into(value, &mut writer)
        }) {
            Ok(length) => writer.write_all(
                &u64::try_from(length)
                    .map_err(std::io::Error::other)?
                    .to_le_bytes(),
            ),
            Err(original) => {
                encoding = Err(original);
                Ok(())
            }
        }
    })
    .map_err(|error| WorldStateSnapshotError::Invalid(error.to_string()))?;
    encoding.map_err(WorldStateSnapshotError::Encoding)?;
    Ok(hash)
}

/// An original acquisition or authentication failure; no caller-authored view is accepted.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RetirementCutError {
    /// The original signed-genesis or source acquisition attempt did not complete.
    #[error(transparent)]
    Chain(#[from] ExecutionAttemptError<ChainReadError>),
    /// The original executed native history did not establish this certificate.
    #[error(transparent)]
    History(#[from] ExecutionAttemptError<iroha_data_model::query::error::QueryExecutionFail>),
    /// Complete World reconstruction or its original physical resources refused.
    #[error(transparent)]
    World(#[from] WorldStateSnapshotError),
    /// No non-genesis executed publication exists for this original State.
    #[error("retirement source cut has no non-genesis executed publication")]
    Unexecuted,
    /// This reader has no original complete-State census binding for the selected profile.
    #[error("retirement source cut uses an unsupported State commitment profile")]
    UnsupportedProfile,
    /// The immutable release catalog could not establish the supported original profile.
    #[error(transparent)]
    Catalog(#[from] crate::runtime::CatalogError),
    /// The original checked encoder refused while comparing a native keyed preimage.
    #[error("retirement keyed census encoding: {0}")]
    Codec(#[source] norito::core::DecodeAttemptError),
    /// Original immutable-tree traversal rejected its exact local table bound.
    #[error(transparent)]
    Tree(#[from] iroha_data_model::sumeragi_finality::keyed_state::KeyedStateError),
    /// Original execution, selected schema or complete native census differs.
    #[error("retirement source cut: {0}")]
    Invalid(&'static str),
    /// Original State pool refused the codec counter/control owner.
    #[error(transparent)]
    DecodeControl(#[from] iroha_allocation::PrepaidSharedError),
    /// The original State pool refused the exact counter backing reservation.
    #[error(transparent)]
    Admission(#[from] iroha_allocation::AllocationRefusal),
}

fn census_scope(
    pool: &iroha_allocation::AllocationBudget,
) -> Result<norito::core::DecodeBudgetContext, RetirementCutError> {
    let mut original = pool.try_reserve(norito::core::DecodeBudgetContext::allocation_layout())?;
    Ok(norito::core::DecodeBudgetContext::from_reservation(
        norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            pool.limit_bytes(),
            usize::MAX,
        ),
        &mut original,
    )?)
}

fn require_supported_profile(
    selection: &iroha_data_model::runtime::definition::RootSelectionV1,
) -> Result<(), RetirementCutError> {
    if selection.commitment_profile
        != crate::runtime::release_catalog()?
            .baseline
            .commitment_profile
        && !crate::runtime::keyed_state::selected(selection)
    {
        return Err(RetirementCutError::UnsupportedProfile);
    }
    Ok(())
}

/// Immutable retirement-only observation of an original certified complete World cut.
/// The view stays private so callers cannot replace its World, network or history fields.
/// This is neither a retirement receipt nor source-removal authority.
pub(crate) struct OriginalRetirementCut<'state> {
    view: StateQueryView<'state>,
    // Keep the original persistent execution tree and its original allocation owner alive.
    _keyed: Option<super::keyed_state::KeyedCut>,
    pool: iroha_allocation::AllocationBudget,
}

impl<'state> OriginalRetirementCut<'state> {
    /// Borrow the internally acquired view without exposing a mutable replacement path.
    pub(crate) fn view(&self) -> &StateQueryView<'state> {
        &self.view
    }
    /// Run bounded census serialization with this cut's original physical State pool.
    pub(crate) fn with_census_scope<T>(
        &self,
        read: impl FnOnce() -> T,
    ) -> Result<T, RetirementCutError> {
        Ok(census_scope(&self.pool)?.with(read))
    }
}

impl State {
    /// Authenticate complete retirement originals against this State's actual native cut.
    /// The existing cold snapshot corridor owns the original finite operation budget and
    /// releases World writers before this immutable observation is returned. Missing native
    /// replay after restoration, changed cuts and original resource refusals remain errors.
    pub(crate) fn runtime_retirement_cut(
        &self,
    ) -> Result<OriginalRetirementCut<'_>, RetirementCutError> {
        let view = self.query_view();
        if view.height() < 2 {
            return Err(RetirementCutError::Unexecuted);
        }
        require_supported_profile(view.world().runtime_selection())?;
        let chain = CertifiedChain::new(&view)?;
        let height =
            std::num::NonZeroUsize::new(view.height()).ok_or(RetirementCutError::Unexecuted)?;
        let certified = chain.certified_from_execution(height, |_, _| Ok(()))?;
        let pool = self.ivm_execution_budget();
        let keyed = census_scope(&pool)?.with(|| {
            if crate::runtime::keyed_state::selected(view.world().runtime_selection()) {
                keyed::authenticate(self, &view, certified.committed()).map(Some)
            } else {
                self.authenticate_runtime_retirement_world(&view, certified.committed())?;
                Ok(None)
            }
        })?;
        drop(chain);
        Ok(OriginalRetirementCut {
            view,
            _keyed: keyed,
            pool,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Actual State owner checks before any retirement source evidence is possible.
    use super::*;

    #[test]
    fn unexecuted_state_cannot_mint_a_retirement_cut() {
        let state = State::new_for_testing(
            super::super::World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        assert!(matches!(
            state.runtime_retirement_cut(),
            Err(RetirementCutError::Unexecuted)
        ));
    }

    #[test]
    fn original_hash_keeps_existing_identity_and_the_original_refusal() {
        struct RefusingOriginal;
        impl norito::SerializePayload for RefusingOriginal {
            fn serialize(
                &self,
                _: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::core::Error> {
                // A real original serializer reservation, refused by the enclosing owner.
                norito::core::reserve_decode_allocation(1)
            }
        }
        let value = crate::runtime::release_catalog().unwrap().baseline.clone();
        assert_eq!(
            original_hash(&value).unwrap(),
            iroha_data_model::sumeragi_finality::world_state_value_hash_v1(&value).unwrap()
        );
        let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
        ));
        let error = zero.with(|| original_hash(&RefusingOriginal)).unwrap_err();
        let WorldStateSnapshotError::Encoding(original) = error else {
            panic!("original encoding owner required")
        };
        assert_eq!(
            original.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
    }

    #[test]
    fn only_known_profiles_can_request_an_original_census_owner() {
        let mut selection = crate::runtime::release_catalog().unwrap().baseline.clone();
        require_supported_profile(&selection).unwrap();
        for profile in [
            iroha_data_model::runtime::keyed_state::profile_id(),
            iroha_data_model::runtime::carrier::keyed::profile_id(),
        ] {
            selection.commitment_profile = profile;
            require_supported_profile(&selection).unwrap();
        }
        selection.commitment_profile.0[0] ^= 1;
        assert!(matches!(
            require_supported_profile(&selection),
            Err(RetirementCutError::UnsupportedProfile)
        ));
    }

    #[test]
    fn census_counter_admission_retains_the_original_pool_refusal() {
        let pool = iroha_allocation::AllocationBudget::new(4096);
        let blocked = pool.try_reserve_bytes(pool.limit_bytes()).unwrap();
        assert!(matches!(
            census_scope(&pool),
            Err(RetirementCutError::Admission(_))
        ));
        drop(blocked);
        let context = census_scope(&pool).unwrap();
        assert!(pool.reserved_bytes() > 0);
        drop(context);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn original_world_binding_refuses_caller_substituted_query_fields() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
                let world = super::super::World::new();
                crate::runtime::activate_additions_for_testing(
                    &world,
                    crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
                )
                .unwrap();
                let mut chain =
                    CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap();
                chain.commit_at(2_000, vec![]);
                let original = chain.state().runtime_retirement_cut().unwrap();
                assert_eq!(original.view().height(), 2);
                let foreign = super::super::World::new();
                let mut edited = chain.state().query_view();
                edited.world = foreign.view();
                let tip = chain.committed(2);
                assert!(
                    chain
                        .state()
                        .authenticate_runtime_retirement_world(&edited, &tip)
                        .is_err()
                );
                assert_eq!(
                    original.view().height(),
                    2,
                    "original observation remains immutable"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
