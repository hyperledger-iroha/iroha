//! Native role-11 custody over a certified test chain, for runtime tests outside Core.
//!
//! The chain is a [`CertifiedTestChain`]: a signed genesis with a fixed four-validator committee,
//! blocks built, executed and applied through the node's block path, each certified by a real BLS
//! `CommitQC`, so the finalized custody and Check readers accept it as they accept a running
//! node's. The custody policy and enrollment are ordinary signed transactions of the custody
//! manager; only the initial accounts, provider owner and permissions are World setup.
//!
//! Accounts by seed ([`StreamTokenRuntimeTestFixtureV1::key`]): `1` manages custody, `2` owns the
//! provider and operates it, `3` observes (Checks), `4` is the role signer key and `7` the
//! independent attester. [`StreamTokenRuntimeTestFixtureV1::new_at`] commits the policy one
//! second before its reference time and the enrollment (a statement issued then) at it, so the
//! fixture's history is never in the future of a wall clock the reference time was read from,
//! while a clock uncertainty reaching back before the reference time precedes the statement.

use std::sync::Arc;

use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    isi::{InstructionBox, Revoke, sorafs::MutateSorafsStreamTokenCustody},
    permission::{Permission, Permissions},
    sorafs::{capacity::ProviderId, stream_token_custody::SorafsStreamTokenCustodyActionV1},
    transaction::SignedTransaction,
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    custody_control::SignerCustodyPolicyV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};

use crate::{
    query::{signer_check::fixture, stream_token_custody::read_stream_token_custody_control_at_v1},
    state::{State, StateReadOnly, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

/// Lifetime of the fixture's signed custody statement (the policy's maximum validity).
const STATEMENT_VALIDITY_MS: u64 = 120_000;
/// Lifetime of the fixture's custody policy around its reference time.
const POLICY_WINDOW_MS: u64 = 60 * 60 * 1000;

/// A certified chain whose provider has an enrolled role-11 custody (see the module
/// documentation).
pub struct StreamTokenRuntimeTestFixtureV1 {
    chain: CertifiedTestChain,
    /// The chain's State.
    pub state: Arc<State>,
    /// The provider whose stream tokens the custody signs.
    pub provider: ProviderId,
    /// The governed custody policy (binding and independent attestation trust).
    pub policy: SignerCustodyPolicyV1,
    /// The canonical enrolled custody record (`SignerCustodyRecordV1`).
    pub record: Vec<u8>,
}

impl core::fmt::Debug for StreamTokenRuntimeTestFixtureV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StreamTokenRuntimeTestFixtureV1")
            .field("chain", &self.chain)
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl StreamTokenRuntimeTestFixtureV1 {
    /// The deterministic Ed25519 key of `seed` (see the module documentation for the roles).
    #[must_use]
    pub fn key(seed: u8) -> KeyPair {
        fixture::key(seed)
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(Self::key(seed).public_key().clone())
    }

    /// A chain whose custody policy is committed at `now_ms - 1000` and whose enrollment
    /// (a statement issued then, valid for two minutes) at `now_ms`; the policy is active for
    /// an hour on either side of `now_ms`.
    ///
    /// # Panics
    /// The chain does not start or a setup transaction fails (fixture misuse).
    #[must_use]
    pub fn new_at(now_ms: u64) -> Self {
        let provider = ProviderId::new([19; 32]);
        let mut world = World::new();
        for seed in 1..=3 {
            let (id, value) = Account::new(Self::account(seed))
                .build(&Self::account(1))
                .into_key_value();
            world.accounts.insert(id, value);
        }
        world.provider_owners.insert(provider, Self::account(2));
        for (seed, permission) in [
            (
                1,
                Permission::from(CanManageSorafsStreamTokenCustody {
                    provider_id: provider,
                }),
            ),
            (
                2,
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
            (
                3,
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
        ] {
            let mut permissions = Permissions::new();
            permissions.insert(permission);
            world
                .account_permissions
                .insert(Self::account(seed), permissions);
        }
        let configured_at = now_ms.saturating_sub(1_000);
        let enrolled_at = now_ms;
        let chain =
            CertifiedTestChain::start(TestChainConfig::new(world, now_ms.saturating_sub(2_000)))
                .map_err(|failure| failure.error)
                .expect("the certified fixture chain starts");
        let state = Arc::clone(chain.state());
        let policy = SignerCustodyPolicyV1 {
            binding: SignerCustodyBindingV1 {
                chain_id: state.view().chain_id().to_string(),
                network_id: *state.network_id_ref().as_bytes(),
                runtime_handle: "software://sorafs/stream-token/primary".into(),
                key_handle: "software://sorafs/stream-token/key-1".into(),
                service_id: "stream-token-service".into(),
                administrator_id: "stream-token-admin".into(),
                role: SignerRoleV1::StreamToken,
                purpose: SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: Self::key(4).public_key().clone(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [5; 32],
            },
            attester_authority: SignerCustodyAuthorityV1 {
                service_id: "custody-service".into(),
                administrator_id: "custody-admin".into(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [6; 32],
            },
            attester_public_key: Self::key(7).public_key().clone(),
            active_from_unix_ms: now_ms.saturating_sub(POLICY_WINDOW_MS),
            active_until_unix_ms: now_ms.saturating_add(POLICY_WINDOW_MS),
            max_validity_ms: STATEMENT_VALIDITY_MS,
            max_anchor_age_ms: 60_000,
        };
        let mut fixture = Self {
            chain,
            state,
            provider,
            policy,
            record: Vec::new(),
        };
        let configure = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 0,
            expected_digest: [0; 32],
            action: SorafsStreamTokenCustodyActionV1::Configure(
                norito::encode_canonical(&fixture.policy).expect("policy encodes"),
            ),
        };
        assert!(
            fixture.commit_instruction(configure.into(), 1, configured_at),
            "the fixture custody policy is configured"
        );
        let height = fixture.state.view().height() as u64;
        let current = read_stream_token_custody_control_at_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            height,
        )
        .expect("custody control reads")
        .expect("custody control is configured");
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: fixture.policy.binding.clone(),
            authority: fixture.policy.attester_authority.clone(),
            anchor: current.anchor,
            sequence: current.state.next_sequence,
            predecessor_digest: current.state.predecessor_digest,
            issued_at_unix_ms: enrolled_at,
            expires_at_unix_ms: enrolled_at.saturating_add(STATEMENT_VALIDITY_MS),
            evidence_digest: [8; 32],
            revoked: false,
        };
        let signature = Signature::try_new(
            Self::key(7).private_key(),
            &statement.signing_payload().expect("statement payload"),
        )
        .expect("attester signature");
        let enrollment = SignerCustodyRecordV1 {
            statement,
            attestation: signature
                .payload()
                .try_into()
                .expect("Ed25519 signature length"),
        };
        fixture.record = norito::encode_canonical(&enrollment).expect("record encodes");
        let enroll = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 1,
            expected_digest: current.anchor.state_digest,
            action: SorafsStreamTokenCustodyActionV1::Enroll(fixture.record.clone()),
        };
        assert!(
            fixture.commit_instruction(enroll.into(), 1, enrolled_at),
            "the fixture custody record enrolls"
        );
        fixture
    }

    /// The certified chain.
    #[must_use]
    pub fn chain(&self) -> &CertifiedTestChain {
        &self.chain
    }

    /// Commit `instruction`, signed by `key(seed)`, in one certified block at `now_ms`; whether
    /// it executed successfully.
    pub fn commit_instruction(
        &mut self,
        instruction: InstructionBox,
        seed: u8,
        now_ms: u64,
    ) -> bool {
        let signed = fixture::sign(&self.state, instruction, seed, now_ms);
        self.commit_signed(signed, now_ms)
    }

    /// Commit an already signed transaction in one certified block at `now_ms` (or the earliest
    /// later canonical time); whether it executed successfully.
    ///
    /// # Panics
    /// The transaction is not accepted on this chain (fixture misuse).
    pub fn commit_signed(&mut self, transaction: SignedTransaction, now_ms: u64) -> bool {
        self.chain.commit_at(now_ms, vec![transaction]) == [true]
    }

    /// Revoke the observer's Check permission (`observer`) or the operator's Operate permission
    /// in one certified block at `now_ms`; whether it executed. The holder renounces it (the
    /// executor lets a holder revoke a permission it holds).
    pub fn revoke_runtime_permission(&mut self, observer: bool, now_ms: u64) -> bool {
        let (permission, seed) = if observer {
            (
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: self.provider,
                }),
                3,
            )
        } else {
            (
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: self.provider,
                }),
                2,
            )
        };
        self.commit_instruction(
            Revoke::account_permission(permission, Self::account(seed)).into(),
            seed,
            now_ms,
        )
    }

    /// Drop the certified Kura frame (block and commit certificate) of `height`, keeping only
    /// its hash: the height is committed but no longer has signer finality on this node.
    ///
    /// # Errors
    /// The height is not stored or Kura cannot rewrite its index.
    pub fn remove_finality_for_test(&self, height: u64) -> Result<(), crate::kura::Error> {
        let height = usize::try_from(height)
            .ok()
            .and_then(core::num::NonZeroUsize::new)
            .ok_or(crate::kura::Error::OutOfBoundsBlockRead {
                start_block_height: height,
                block_count: self.state.view().height(),
            })?;
        self.state.kura().force_hash_only_block_for_testing(height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::stream_token_authority::observation::capture_stream_token_authority_v1;

    const T: u64 = 1_700_000_000_000;

    #[test]
    fn custody_is_enrolled_final_and_current_at_the_reference_time() {
        let fixture = StreamTokenRuntimeTestFixtureV1::new_at(T);
        let view = fixture.state.view();
        // Genesis, the policy and the enrollment.
        assert_eq!(view.height(), 3);
        let current = capture_stream_token_authority_v1(&view, &fixture.policy.binding, [0; 32])
            .expect("current custody is final");
        assert_eq!(current.floor.height, 3);
        assert_eq!(
            current.operator,
            StreamTokenRuntimeTestFixtureV1::account(2)
        );
        let head = current.control.active_head.expect("enrolled");
        let record: SignerCustodyRecordV1 =
            norito::decode_from_bytes(&fixture.record).expect("record decodes");
        assert_eq!(record.statement.issued_at_unix_ms, T);
        assert_eq!(head.sequence, record.statement.sequence);
        assert_eq!(fixture.chain().committed(3).block_time_ms(), T);
        assert_eq!(fixture.chain().committed(2).block_time_ms(), T - 1_000);
    }

    #[test]
    fn revocations_and_removed_finality_close_the_runtime_authority() {
        let mut fixture = StreamTokenRuntimeTestFixtureV1::new_at(T);
        assert!(fixture.revoke_runtime_permission(true, T + 1_000));
        assert!(fixture.revoke_runtime_permission(false, T + 2_000));
        // Revoking again executes and fails: the permission is gone.
        assert!(!fixture.revoke_runtime_permission(false, T + 3_000));
        let view = fixture.state.view();
        let current = capture_stream_token_authority_v1(&view, &fixture.policy.binding, [0; 32])
            .expect("custody itself is unchanged");
        let anchor = current.floor.height;
        drop(view);
        fixture
            .remove_finality_for_test(anchor)
            .expect("frame dropped");
        assert!(
            capture_stream_token_authority_v1(
                &fixture.state.view(),
                &fixture.policy.binding,
                [0; 32]
            )
            .is_err()
        );
        assert!(fixture.remove_finality_for_test(0).is_err());
    }
}
