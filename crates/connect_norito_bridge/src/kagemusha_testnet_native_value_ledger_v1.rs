//! Rust-only native host ledger for exact, proof-backed experimental testnet mint credits.
//!
//! A native host installs the signed Experimental release and durable proof owner first,
//! then opens this ledger in a distinct private directory. Only the opaque value admission
//! returned under that owner's lock can add a credit. No C/JNI archive or caller-supplied
//! finality coordinates can authorize a ledger write.

use std::{
    mem::{align_of, size_of},
    path::Path,
    ptr, slice,
    sync::{Mutex, OnceLock},
};

use iroha_core::zk::kagemusha_v1_recursion::{
    KagemushaTestnetMintCreditLedgerV1, KagemushaTestnetMintLedgerCreditV1,
};
use libc::{c_int, c_uchar};

use crate::{
    ERR_BUFFER_TOO_SMALL, ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1, ERR_KAGEMUSHA_V1, ERR_NULL_PTR,
    kagemusha_testnet_observation_v1::{
        KagemushaTestnetDurableObservationModeV1, with_kagemusha_testnet_durable_credit_owner_v1,
    },
    kagemusha_testnet_publication_v1::{
        TestnetPublicationPermitV1, catch_testnet_dispatch_panic_v1, testnet_publication_gate_v1,
    },
};

/// Full caller-provided capacity for one canonical counted-credit inspection archive.
pub const KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1: usize = 512;

/// Copyable evidence of one credit durably counted by the native testnet ledger.
///
/// Only a successful live native call attests that a ledger write was acknowledged. These
/// bytes can be copied or forged after return and are never a payment or production capability.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1")]
pub struct KagemushaTestnetMintLedgerCreditArchiveV1 {
    version: u16,
    hardware_qualified: bool,
    network_id: [u8; 32],
    release_id: [u8; 32],
    release_attestation_digest: [u8; 32],
    asset_identity_digest: [u8; 32],
    asset_incarnation: [u8; 32],
    asset_scale: u32,
    liability_pool_id: [u8; 32],
    operation_id: [u8; 32],
    credit_id: [u8; 32],
    amount: u128,
    total_admitted: u128,
}

static TESTNET_VALUE_LEDGER_V1: OnceLock<Mutex<KagemushaTestnetNativeValueLedgerV1>> =
    OnceLock::new();
static TESTNET_VALUE_LEDGER_INSTALL_LOCK_V1: Mutex<()> = Mutex::new(());

/// One exclusive experimental mint-credit ledger under the installed durable proof owner.
///
/// This native host type counts real finalized top-ups in the exact signed testnet scope.
/// It does not bind a wallet account, authorize peer spending, or enter production monetary
/// state. The returned credit is an inspection result, never a hardware credential.
pub(crate) struct KagemushaTestnetNativeValueLedgerV1 {
    ledger: KagemushaTestnetMintCreditLedgerV1,
}

impl KagemushaTestnetNativeValueLedgerV1 {
    /// Create a private ledger for the currently installed durable proof owner's signed scope.
    ///
    /// # Errors
    /// Rejects absent/process-only owner, unsafe or existing path, and uncertain storage.
    pub(crate) fn create_new(
        publication: &TestnetPublicationPermitV1<'_>,
        path: &Path,
    ) -> Result<Self, String> {
        with_kagemusha_testnet_durable_credit_owner_v1(publication, |owner| {
            KagemushaTestnetMintCreditLedgerV1::create_new(path, owner)
                .map(|ledger| Self { ledger })
                .map_err(|error| {
                    format!("KAGEMUSHA testnet value ledger creation rejected: {error}")
                })
        })
    }

    /// Recover every ledger credit against the owner's reverified original Applied mint proof.
    ///
    /// The signed Experimental release, private observation journal, and independently
    /// supplied finality anchors must already have been recovered by the native host.
    ///
    /// # Errors
    /// Rejects absent/process-only owner, changed proof/reservation/finality evidence,
    /// malformed or conflicting ledger records, duplicate credits, or unsafe storage.
    /// A valid older complete WAL prefix remains undetectable without an external trusted head.
    pub(crate) fn open_existing(
        publication: &TestnetPublicationPermitV1<'_>,
        path: &Path,
    ) -> Result<Self, String> {
        with_kagemusha_testnet_durable_credit_owner_v1(publication, |owner| {
            KagemushaTestnetMintCreditLedgerV1::open_existing(path, owner)
                .map(|ledger| Self { ledger })
                .map_err(|error| {
                    format!("KAGEMUSHA testnet value ledger recovery rejected: {error}")
                })
        })
    }

    /// Durably count one owner-retained Applied top-up and paired MintFold by operation ID.
    ///
    /// The caller supplies no amount, proof verdict, reserve, or finality anchor. An exact
    /// retry returns the original counted credit without appending a second ledger record.
    ///
    /// # Errors
    /// Rejects absent/process-only owner, unobserved or changed proof, duplicate credit,
    /// scope mismatch, or uncertain ledger storage.
    pub(crate) fn credit_finalized_top_up(
        &mut self,
        publication: &TestnetPublicationPermitV1<'_>,
        operation_id: [u8; 32],
    ) -> Result<KagemushaTestnetMintLedgerCreditV1, String> {
        with_kagemusha_testnet_durable_credit_owner_v1(publication, |owner| {
            let admission = owner
                .admit_finalized_testnet_value(operation_id)
                .map_err(|error| format!("KAGEMUSHA testnet value admission rejected: {error}"))?;
            self.ledger
                .credit(&admission)
                .map_err(|error| format!("KAGEMUSHA testnet ledger credit rejected: {error}"))
        })
    }

    /// Return the total amount of durably counted testnet mint credits.
    #[must_use]
    pub const fn total_admitted(&self) -> u128 {
        self.ledger.total_admitted()
    }
}

/// Install the one native experimental value ledger after the durable proof owner.
///
/// The journal path and create/recover mode must come from trusted native configuration,
/// never from a C/JNI request or Torii operation status. A second installer is rejected
/// before it can create an orphan journal.
///
/// # Errors
/// Rejects an absent/process-only proof owner, duplicate installation, unsafe path, changed
/// replay evidence, or uncertain storage.
pub(crate) fn install_kagemusha_testnet_native_value_ledger_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    path: &Path,
    mode: KagemushaTestnetDurableObservationModeV1,
) -> Result<(), String> {
    publication.require_valid()?;
    let _install_guard = TESTNET_VALUE_LEDGER_INSTALL_LOCK_V1
        .lock()
        .map_err(|_| "KAGEMUSHA native testnet value-ledger install lock is poisoned".to_owned())?;
    if TESTNET_VALUE_LEDGER_V1.get().is_some() {
        return Err("KAGEMUSHA native testnet value ledger is already installed".to_owned());
    }
    let ledger = match mode {
        KagemushaTestnetDurableObservationModeV1::Create => {
            KagemushaTestnetNativeValueLedgerV1::create_new(publication, path)
        }
        KagemushaTestnetDurableObservationModeV1::Recover => {
            KagemushaTestnetNativeValueLedgerV1::open_existing(publication, path)
        }
    }?;
    TESTNET_VALUE_LEDGER_V1
        .set(Mutex::new(ledger))
        .map_err(|_| "KAGEMUSHA native testnet value ledger is already installed".to_owned())
}

/// Durably count one finalized testnet top-up in the installed native ledger.
///
/// Only the operation ID crosses this boundary. The installed proof owner rederives the
/// opaque admission from its own reservation, paired proof, signed release and pinned finality
/// while the native ledger holds its exclusive journal. The returned record is inspectable,
/// not a spend credential.
///
/// # Errors
/// Rejects missing installation/Applied proof, duplicate or changed credit, wrong scope,
/// or uncertain ledger storage.
pub fn credit_kagemusha_testnet_native_value_v1(
    operation_id: [u8; 32],
) -> Result<(KagemushaTestnetMintLedgerCreditV1, u128), String> {
    testnet_publication_gate_v1().with_dispatch(|publication| {
        credit_kagemusha_testnet_native_value_under_publication_v1(publication, operation_id)
    })
}

pub(crate) fn credit_kagemusha_testnet_native_value_under_publication_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    operation_id: [u8; 32],
) -> Result<(KagemushaTestnetMintLedgerCreditV1, u128), String> {
    publication.require_valid()?;
    let installed = TESTNET_VALUE_LEDGER_V1
        .get()
        .ok_or_else(|| "KAGEMUSHA native testnet value ledger is unavailable".to_owned())?;
    let mut installed = installed
        .lock()
        .map_err(|_| "KAGEMUSHA native testnet value ledger is poisoned".to_owned())?;
    let credit = installed.credit_finalized_top_up(publication, operation_id)?;
    Ok((credit, installed.total_admitted()))
}

/// Credit one owner-retained finalized testnet top-up and return copyable inspection facts.
///
/// The caller supplies only the 32-byte operation ID and full output capacity. The native
/// owner and ledger make the monetary decision before any archive is returned; submitted
/// archives, amounts, finality coordinates and proof verdicts are never accepted as inputs.
///
/// # Safety
/// Non-null pointers must reference accessible declared spans. The output must be writable;
/// `output_len` must be naturally aligned and disjoint from input and output spans.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_value_credit_v1(
    operation_id_ptr: *const c_uchar,
    operation_id_len: usize,
    output_ptr: *mut c_uchar,
    output_capacity: usize,
    output_len: *mut usize,
) -> c_int {
    if output_len.is_null() {
        return ERR_NULL_PTR;
    }
    let length_start = output_len as usize;
    let Some(length_end) = length_start.checked_add(size_of::<usize>()) else {
        return ERR_KAGEMUSHA_V1;
    };
    let input_start = operation_id_ptr as usize;
    let output_start = output_ptr as usize;
    let Some(input_end) = input_start.checked_add(operation_id_len) else {
        return ERR_KAGEMUSHA_V1;
    };
    let Some(output_end) = output_start.checked_add(output_capacity) else {
        return ERR_KAGEMUSHA_V1;
    };
    if !length_start.is_multiple_of(align_of::<usize>())
        || (input_start < length_end && length_start < input_end)
        || (output_start < length_end && length_start < output_end)
        || (input_start < output_end && output_start < input_end)
    {
        return ERR_KAGEMUSHA_V1;
    }
    unsafe { *output_len = 0 };
    if operation_id_ptr.is_null() || output_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if operation_id_len != 32 {
        return ERR_KAGEMUSHA_V1;
    }
    if output_capacity < KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1 {
        return ERR_BUFFER_TOO_SMALL;
    }
    // Serialize the entire admission and ledger write with complete host publication.
    let Ok(_publication) = testnet_publication_gate_v1().dispatch() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let operation_id: [u8; 32] = unsafe { slice::from_raw_parts(operation_id_ptr, 32) }
        .try_into()
        .expect("fixed operation ID length");
    let encoded = catch_testnet_dispatch_panic_v1(&_publication, || {
        let (credit, total_admitted) = credit_kagemusha_testnet_native_value_under_publication_v1(
            &_publication.permit(),
            operation_id,
        )
        .map_err(|_| ())?;
        let scope = credit.scope();
        let archive = KagemushaTestnetMintLedgerCreditArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: scope.network_id(),
            release_id: scope.release_id(),
            release_attestation_digest: scope.release_attestation_digest(),
            asset_identity_digest: scope.asset_identity_digest(),
            asset_incarnation: scope.asset_incarnation(),
            asset_scale: scope.asset_scale(),
            liability_pool_id: scope.liability_pool_id(),
            operation_id: credit.operation_id(),
            credit_id: credit.credit_id(),
            amount: credit.amount(),
            total_admitted,
        };
        let bytes = norito::encode_canonical(&archive).map_err(|_| ())?;
        if bytes.len() > KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1 {
            return Err(());
        }
        Ok(bytes)
    });
    let Ok(Ok(encoded)) = encoded else {
        return if TESTNET_VALUE_LEDGER_V1.get().is_some() {
            ERR_KAGEMUSHA_V1
        } else {
            ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        };
    };
    unsafe {
        ptr::copy_nonoverlapping(encoded.as_ptr(), output_ptr, encoded.len());
        *output_len = encoded.len();
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_credit_permit_rejects_caught_panic_before_looking_up_or_locking_ledger() {
        let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
        let guard = gate.dispatch().unwrap();
        let permit = guard.permit();
        assert!(
            catch_testnet_dispatch_panic_v1(&guard, || panic!("uncertain credit callback"))
                .is_err()
        );
        let error = credit_kagemusha_testnet_native_value_under_publication_v1(&permit, [9; 32])
            .err()
            .unwrap();
        assert_eq!(
            error,
            "KAGEMUSHA testnet publication was permanently revoked"
        );
    }

    #[test]
    fn native_ledger_cannot_start_without_installed_durable_owner() {
        let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
        let publication = gate.exclusive().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("value-ledger");
        assert!(
            KagemushaTestnetNativeValueLedgerV1::create_new(&publication.permit(), &path).is_err()
        );
        assert!(!path.exists());
        assert!(
            KagemushaTestnetNativeValueLedgerV1::open_existing(&publication.permit(), &path)
                .is_err()
        );
    }

    #[test]
    fn native_credit_c_abi_rejects_missing_ledger_and_bad_buffers_without_writing() {
        let operation_id = [0x11_u8; 32];
        let mut output = [0xA5_u8; KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1];
        let mut output_len = usize::MAX;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_value_credit_v1(
                    operation_id.as_ptr(),
                    31,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(output_len, 0);
        assert_eq!(output, [0xA5; KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1]);
        output_len = usize::MAX;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_value_credit_v1(
                    operation_id.as_ptr(),
                    operation_id.len(),
                    output.as_mut_ptr(),
                    output.len() - 1,
                    &mut output_len,
                )
            },
            ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(output_len, 0);
        assert_eq!(output, [0xA5; KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1]);
        output_len = usize::MAX;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_value_credit_v1(
                    operation_id.as_ptr(),
                    operation_id.len(),
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert_eq!(output_len, 0);
        assert_eq!(output, [0xA5; KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1]);
        assert!(credit_kagemusha_testnet_native_value_v1(operation_id).is_err());
    }

    #[test]
    fn native_credit_archive_is_bounded_and_explicitly_unqualified() {
        let record = KagemushaTestnetMintLedgerCreditArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: [1; 32],
            release_id: [2; 32],
            release_attestation_digest: [3; 32],
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
            operation_id: [7; 32],
            credit_id: [8; 32],
            amount: 17,
            total_admitted: 17,
        };
        let bytes = norito::encode_canonical(&record).unwrap();
        assert_eq!(bytes.len(), 356);
        assert_eq!(&bytes[40..48], &[0; 8]);
        assert!(bytes.len() <= KAGEMUSHA_TESTNET_VALUE_CREDIT_MAX_BYTES_V1);
        let decoded: KagemushaTestnetMintLedgerCreditArchiveV1 =
            norito::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, record);
        assert!(!decoded.hardware_qualified);
    }
}
