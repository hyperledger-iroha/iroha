//! Rust-only ordering for an Experimental testnet mint in an ordinary mobile app.
//!
//! This host retains independently configured release and finality roots inside Rust. The
//! application's JNI calls may submit and inspect an operation, but cannot install the owner,
//! transport a private credit opening, select the finality root, or create a counted credit.

use std::path::Path;

use iroha_core::zk::{
    kagemusha_v1_recursion::KagemushaTestnetMintLedgerCreditV1,
    kagemusha_v1_state::MintInboxReservationV1,
};
use iroha_data_model::{
    NetworkId, block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
};

use crate::{
    kagemusha_testnet_finality_chain_v1::verify_kagemusha_testnet_finality_anchor_from_chain_v1,
    kagemusha_testnet_native_mint_runtime_v1::{
        KagemushaTestnetNativeMintInstallV1, KagemushaTestnetNativeMintReservationV1,
        KagemushaTestnetNativeMintRuntimeV1,
    },
    kagemusha_testnet_native_value_ledger_v1::{
        credit_kagemusha_testnet_native_value_v1, install_kagemusha_testnet_native_value_ledger_v1,
    },
    kagemusha_testnet_observation_v1::KagemushaTestnetDurableObservationModeV1,
};

/// One native host that owns the signed release, private reservation, and testnet credit order.
///
/// Installation is process-wide because the proof owner and value ledger each have one durable
/// singleton. It grants no hardware-qualified or production monetary authority.
pub struct KagemushaTestnetNativeMobileHostV1 {
    mint: KagemushaTestnetNativeMintRuntimeV1,
    trusted_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
}

/// Proof that this exact host fsynced a private mint reservation before online submission.
///
/// The token is process-local. A crash requires recovery of the private journal and an exact
/// reservation retry before a new token can be created.
#[must_use]
pub struct KagemushaTestnetNativeReservedMintV1<'a> {
    host: &'a KagemushaTestnetNativeMobileHostV1,
    reservation: KagemushaTestnetNativeMintReservationV1,
}

impl KagemushaTestnetNativeReservedMintV1<'_> {
    /// Return the operation ID to correlate the exact signed top-up submission.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        self.reservation.operation_id()
    }
}

/// Proof that the same reserved mint has an independently rooted signed finality chain.
///
/// This is not a hardware attestation or a spend credential. Native proof observation must
/// still verify the original Applied status and paired MintFold before the ledger can credit.
#[must_use]
pub struct KagemushaTestnetNativePinnedMintV1<'a> {
    host: &'a KagemushaTestnetNativeMobileHostV1,
    operation_id: [u8; 32],
    anchor: KagemushaFinalityTrustAnchorV1,
}

impl KagemushaTestnetNativePinnedMintV1<'_> {
    /// Return the exact reserved top-up operation ID for native observation.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        self.operation_id
    }

    /// Return comparison coordinates derived from the checked signed chain.
    ///
    /// The native observer independently requires this operation's durable finality pin.
    #[must_use]
    pub const fn finality_anchor(&self) -> KagemushaFinalityTrustAnchorV1 {
        self.anchor
    }
}

impl KagemushaTestnetNativeMobileHostV1 {
    /// Install the signed Experimental proof owner and then its separate durable value ledger.
    ///
    /// Every input is trusted native-host configuration or an authenticated release package;
    /// none is accepted through the C/JNI observation boundary. Each private journal has its
    /// own native-trusted create/recover mode. Fresh installation uses Create/Create. Normal
    /// restart uses Recover/Recover. Mixed modes fail closed: a missing ledger may have been
    /// rolled back, and path absence cannot prove it was never created. An interrupted first
    /// installation requires explicit repair backed by a trusted external checkpoint before
    /// this host can restart. If the second installation fails, the process must stop; the
    /// proof owner cannot be reset in place.
    ///
    /// # Errors
    /// Rejects a shared journal path, mismatched mode pair, invalid release, changed private
    /// history, failed ledger replay, or duplicate process installation.
    pub fn install(
        mint_inputs: KagemushaTestnetNativeMintInstallV1<'_>,
        value_ledger_path: &Path,
        trusted_value_ledger_mode: KagemushaTestnetDurableObservationModeV1,
    ) -> Result<Self, String> {
        let trusted_network_id = mint_inputs.trusted_network_id;
        let trusted_first_context_id = mint_inputs.trusted_first_context_id;
        let mint_mode = mint_inputs.mode;
        let mint_journal_path = mint_inputs.journal_path;
        let mint = install_in_order(
            mint_journal_path,
            value_ledger_path,
            mint_mode,
            trusted_value_ledger_mode,
            || KagemushaTestnetNativeMintRuntimeV1::install(mint_inputs),
            || {
                install_kagemusha_testnet_native_value_ledger_v1(
                    value_ledger_path,
                    trusted_value_ledger_mode,
                )
            },
        )?;
        Ok(Self {
            mint,
            trusted_network_id,
            trusted_first_context_id,
        })
    }

    /// Fsync the native-owned private opening before exposing the operation for submission.
    ///
    /// # Errors
    /// Rejects a malformed, conflicting, or non-durable reservation.
    pub fn reserve_before_submission(
        &self,
        reservation: &MintInboxReservationV1,
    ) -> Result<KagemushaTestnetNativeReservedMintV1<'_>, String> {
        Ok(KagemushaTestnetNativeReservedMintV1 {
            host: self,
            reservation: self.mint.reserve_before_submission(reservation)?,
        })
    }

    /// Prepare a private mint record inside the Rust host and fsync it before returning its ID.
    ///
    /// The preparation callback must be the trusted native hardware/proof owner. Its record
    /// contains the confidential credit opening and cannot be supplied by a C/JNI caller.
    ///
    /// # Errors
    /// Propagates preparation, validation, or durable reservation failure without issuing a
    /// submission token.
    pub fn prepare_and_reserve(
        &self,
        prepare: impl FnOnce() -> Result<MintInboxReservationV1, String>,
    ) -> Result<KagemushaTestnetNativeReservedMintV1<'_>, String> {
        prepare_then_reserve(prepare, |reservation| {
            self.reserve_before_submission(reservation)
        })
    }

    /// Authenticate and durably pin a consecutive signed chain for a reserved operation.
    ///
    /// The first context and network remain native-host pins. Returned coordinates may be
    /// passed to the existing JNI observer only as comparison evidence; the observer cannot
    /// install or replace the private pin.
    ///
    /// # Errors
    /// Rejects a foreign reservation token, invalid chain, missing native reservation, or a
    /// replacement finality pin.
    pub fn pin_signed_finality<'a>(
        &'a self,
        reserved: &KagemushaTestnetNativeReservedMintV1<'a>,
        chain_json: &[u8],
    ) -> Result<KagemushaTestnetNativePinnedMintV1<'a>, String> {
        if !std::ptr::eq(self, reserved.host) {
            return Err("testnet mint reservation belongs to another native host".to_owned());
        }
        let anchor = verify_then_pin(
            self.trusted_network_id,
            self.trusted_first_context_id,
            chain_json,
            || {
                self.mint
                    .pin_finality_chain(&reserved.reservation, chain_json)
            },
        )?;
        Ok(KagemushaTestnetNativePinnedMintV1 {
            host: self,
            operation_id: reserved.operation_id(),
            anchor,
        })
    }

    /// Count one pinned, owner-observed Applied top-up after its paired MintFold was verified.
    ///
    /// The operation ID only locates the native owner's original proof and private opening.
    /// The ledger rederives opaque admission under the owner lock and fsyncs a unique positive
    /// credit. Calling this before successful proof observation fails closed.
    ///
    /// # Errors
    /// Rejects a foreign token, absent or changed observation, duplicate credit, scope change,
    /// or uncertain ledger storage.
    pub fn credit_observed_top_up(
        &self,
        pinned: &KagemushaTestnetNativePinnedMintV1<'_>,
    ) -> Result<(KagemushaTestnetMintLedgerCreditV1, u128), String> {
        if !std::ptr::eq(self, pinned.host) {
            return Err("testnet mint finality belongs to another native host".to_owned());
        }
        credit_kagemusha_testnet_native_value_v1(pinned.operation_id)
    }
}

fn prepare_then_reserve<T, R>(
    prepare: impl FnOnce() -> Result<T, String>,
    reserve: impl FnOnce(&T) -> Result<R, String>,
) -> Result<R, String> {
    let prepared = prepare()?;
    reserve(&prepared)
}

fn verify_then_pin(
    network_id: NetworkId,
    first_context_id: HeightContextId,
    chain_json: &[u8],
    pin: impl FnOnce() -> Result<bool, String>,
) -> Result<KagemushaFinalityTrustAnchorV1, String> {
    let anchor = verify_kagemusha_testnet_finality_anchor_from_chain_v1(
        network_id,
        first_context_id,
        chain_json,
    )?;
    pin()?;
    Ok(anchor)
}

fn install_in_order<T>(
    mint_journal_path: &Path,
    value_ledger_path: &Path,
    mint_mode: KagemushaTestnetDurableObservationModeV1,
    ledger_mode: KagemushaTestnetDurableObservationModeV1,
    install_mint: impl FnOnce() -> Result<T, String>,
    install_ledger: impl FnOnce() -> Result<(), String>,
) -> Result<T, String> {
    if mint_journal_path == value_ledger_path {
        return Err("testnet mint and value journals must have distinct paths".to_owned());
    }
    if mint_mode != ledger_mode {
        return Err(
            "testnet mint and value journals require matching create/recover modes".to_owned(),
        );
    }
    let mint = install_mint()?;
    install_ledger()?;
    Ok(mint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use std::{cell::RefCell, path::PathBuf};

    const CREATE: KagemushaTestnetDurableObservationModeV1 =
        KagemushaTestnetDurableObservationModeV1::Create;
    const RECOVER: KagemushaTestnetDurableObservationModeV1 =
        KagemushaTestnetDurableObservationModeV1::Recover;

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])))
    }

    fn first_context() -> HeightContextId {
        HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([5; 32])))
    }

    #[test]
    fn host_install_orders_signed_owner_before_ledger_and_rejects_shared_path() {
        let calls = RefCell::new(Vec::new());
        let mint = PathBuf::from("/private/mint-journal");
        let ledger = PathBuf::from("/private/value-ledger");
        assert_eq!(
            install_in_order(
                &mint,
                &ledger,
                CREATE,
                CREATE,
                || {
                    calls.borrow_mut().push("mint");
                    Ok(7_u8)
                },
                || {
                    calls.borrow_mut().push("ledger");
                    Ok(())
                }
            ),
            Ok(7)
        );
        assert_eq!(*calls.borrow(), ["mint", "ledger"]);
        calls.borrow_mut().clear();
        assert!(
            install_in_order(
                &mint,
                &mint,
                CREATE,
                CREATE,
                || {
                    calls.borrow_mut().push("mint");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("ledger");
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(calls.borrow().is_empty());
    }

    #[test]
    fn host_install_does_not_open_ledger_after_failed_signed_owner() {
        let calls = RefCell::new(Vec::new());
        let result = install_in_order(
            Path::new("/private/mint-journal"),
            Path::new("/private/value-ledger"),
            CREATE,
            CREATE,
            || {
                calls.borrow_mut().push("mint");
                Err::<(), _>("unsigned release".to_owned())
            },
            || {
                calls.borrow_mut().push("ledger");
                Ok(())
            },
        );
        assert_eq!(result, Err("unsigned release".to_owned()));
        assert_eq!(*calls.borrow(), ["mint"]);
    }

    #[test]
    fn host_install_does_not_issue_handle_after_ledger_failure() {
        let calls = RefCell::new(Vec::new());
        let result = install_in_order(
            Path::new("/private/mint-journal"),
            Path::new("/private/value-ledger"),
            CREATE,
            CREATE,
            || {
                calls.borrow_mut().push("mint");
                Ok(7_u8)
            },
            || {
                calls.borrow_mut().push("ledger");
                Err("ledger replay failed".to_owned())
            },
        );
        assert_eq!(result, Err("ledger replay failed".to_owned()));
        assert_eq!(*calls.borrow(), ["mint", "ledger"]);
    }

    #[test]
    fn mixed_recovery_modes_are_rejected_before_install() {
        let calls = RefCell::new(Vec::new());
        let result = install_in_order(
            Path::new("/private/mint-journal"),
            Path::new("/private/value-ledger"),
            RECOVER,
            CREATE,
            || {
                calls.borrow_mut().push("mint");
                Ok(7_u8)
            },
            || {
                calls.borrow_mut().push("ledger");
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err("testnet mint and value journals require matching create/recover modes".to_owned())
        );
        assert!(calls.borrow().is_empty());
        calls.borrow_mut().clear();
        let result = install_in_order(
            Path::new("/private/mint-journal"),
            Path::new("/private/value-ledger"),
            RECOVER,
            RECOVER,
            || {
                calls.borrow_mut().push("recover signed mint journal");
                Ok(7_u8)
            },
            || {
                calls.borrow_mut().push("recover counted ledger");
                Ok(())
            },
        );
        assert_eq!(result, Ok(7));
        assert_eq!(
            *calls.borrow(),
            ["recover signed mint journal", "recover counted ledger"]
        );
    }

    #[test]
    fn existing_ledger_without_mint_journal_is_rejected_before_install() {
        let calls = RefCell::new(Vec::new());
        let result = install_in_order(
            Path::new("/private/mint-journal"),
            Path::new("/private/value-ledger"),
            CREATE,
            RECOVER,
            || {
                calls.borrow_mut().push("mint");
                Ok(7_u8)
            },
            || {
                calls.borrow_mut().push("ledger");
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err("testnet mint and value journals require matching create/recover modes".to_owned())
        );
        assert!(calls.borrow().is_empty());
    }

    #[test]
    fn invalid_signed_chain_cannot_reach_native_pin() {
        let pin_called = RefCell::new(false);
        assert!(
            verify_then_pin(network(), first_context(), b"[]", || {
                *pin_called.borrow_mut() = true;
                Ok(true)
            })
            .is_err()
        );
        assert!(!*pin_called.borrow());
    }

    #[test]
    fn private_preparation_must_succeed_before_reservation() {
        let calls = RefCell::new(Vec::new());
        let result = prepare_then_reserve(
            || {
                calls.borrow_mut().push("prepare");
                Err::<u8, _>("private opening unavailable".to_owned())
            },
            |_| {
                calls.borrow_mut().push("reserve");
                Ok(())
            },
        );
        assert_eq!(result, Err("private opening unavailable".to_owned()));
        assert_eq!(*calls.borrow(), ["prepare"]);
        calls.borrow_mut().clear();
        assert_eq!(
            prepare_then_reserve(
                || {
                    calls.borrow_mut().push("prepare");
                    Ok(7_u8)
                },
                |record| {
                    calls.borrow_mut().push("reserve");
                    Ok(*record)
                }
            ),
            Ok(7)
        );
        assert_eq!(*calls.borrow(), ["prepare", "reserve"]);
    }
}
