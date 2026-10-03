//! Dedicated ordinary Mint funding over the actual installed Main/AccountClient owners.
//! Frames carry only amount or complete originals. No frame provides a key/root/clock/grant.
use super::*;
use iroha_core_zk::kagemusha_v1_state::KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 as CLOCK_MAX;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1 as C_MAX,
    KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1 as CONTROL_MAX,
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 as FINALIZED_MAX,
    KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1 as SELECTION_MAX,
    KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1 as DECISION_MAX,
    KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1 as REQUEST_MAX,
    KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1 as NODE_MAX,
};
const DATA_MAX: usize = 128 * 1024;
const PLATFORM_MAX: usize = 4096;
const FRAME_MAX: usize = NODE_MAX + CLOCK_MAX + CONTROL_MAX + DECISION_MAX + DATA_MAX + 4096;
/// Sole first-release Mint funding request. The serializable frame is data, never authority.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingRequestV1")]
pub struct KagemushaOrdinaryNativeMintFundingRequestV1 {
    /// Exactly one.
    pub version: u16,
    /// 1 prepare positive LE128 amount; 2 platform fence; 3 retain raw platform original;
    /// 4 real Mint113 proof/fsync; 5 Native Ed consent; 6 prepare exclusive pre-debit intent;
    /// 7 pre-debit HTTP fence; 8 retain four Core originals plus complete Node packet;
    /// 9 Native quoted transaction/signature/fsync; 10 submit same retained transaction;
    /// 11 genuine signed finality read/intake; 12 read-only platform recovery;
    /// 13 genuine same-S/W clock/account refresh; 14 read-only original Core recovery;
    /// 15 read actual retained platform counter floor for the same Mint operation;
    /// 16 acknowledge an already retained raw platform original within its unchanged window;
    /// 17 read the actual private WAL funding stage, without invoking a key or transport.
    pub phase: u8,
    /// Exact already opened ordinary descriptor, not an installer selector.
    pub core_handle: u64,
    /// Sole full original roles selected by the phase.
    pub originals: Vec<Vec<u8>>,
}
/// Data projections only after the actual retained Native invocation.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingResponseV1")]
pub struct KagemushaOrdinaryNativeMintFundingResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same selected request phase.
    pub phase: u8,
    /// Same retained opened descriptor.
    pub core_handle: u64,
    /// Native-owned originals; submission confirmation/pending finality cannot grant funds.
    pub fields: Vec<Vec<u8>>,
}
fn require_shape(phase: u8, handle: u64, lengths: &[usize]) -> Result<(), Error> {
    if handle == 0 {
        return Err(Error::Rejected);
    }
    let valid = match (phase, lengths) {
        (1, [16]) => true,
        (3, [raw]) => (1..=PLATFORM_MAX).contains(raw),
        (8, [decision, clock, control, data, node]) => {
            (1..=DECISION_MAX).contains(decision)
                && (1..=CLOCK_MAX).contains(clock)
                && (1..=CONTROL_MAX).contains(control)
                && (1..=DATA_MAX).contains(data)
                && (1..=NODE_MAX).contains(node)
        }
        (2 | 4 | 5 | 6 | 7 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17, []) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn nonzero32(raw: &[u8]) -> bool {
    raw.len() == 32 && raw.iter().any(|b| *b != 0)
}
fn require_fields(phase: u8, fields: &[Vec<u8>]) -> Result<(), Error> {
    let valid = match (phase, fields) {
        (1, [operation, message, c]) => {
            nonzero32(operation)
                && (1..=PLATFORM_MAX).contains(&message.len())
                && (1..=C_MAX).contains(&c.len())
        }
        (4 | 9, [digest]) => nonzero32(digest),
        (5, [signature]) => signature.len() == 64,
        (
            6 | 7 | 14,
            [
                selection,
                request,
                signature,
                c,
                pi,
                clock,
                original_control,
                current_control,
            ],
        ) => {
            (1..=SELECTION_MAX).contains(&selection.len())
                && (1..=REQUEST_MAX).contains(&request.len())
                && signature.len() == 64
                && (1..=C_MAX).contains(&c.len())
                && pi.len() <= 4096
                && (1..=CLOCK_MAX).contains(&clock.len())
                && (1..=CONTROL_MAX).contains(&original_control.len())
                && (1..=CONTROL_MAX).contains(&current_control.len())
        }
        (11, [status, raw]) => {
            matches!((status.as_slice(), raw.len()), ([0], 0))
                || status == &[1] && (1..=FINALIZED_MAX).contains(&raw.len())
        }
        (12, [status, operation, message, c, raw]) => {
            matches!(status.as_slice(), [0] | [1] | [2] | [3])
                && nonzero32(operation)
                && (1..=PLATFORM_MAX).contains(&message.len())
                && (1..=C_MAX).contains(&c.len())
                && raw.len() <= PLATFORM_MAX
                && if status == &[0] || status == &[1] {
                    raw.is_empty()
                } else {
                    !raw.is_empty()
                }
        }
        (15, [platform, floor]) => {
            matches!((platform.as_slice(), floor.len()), ([5], 0) | ([4], 4))
        }
        (17, [operation, stage]) => nonzero32(operation) && stage.len() == 1 && stage[0] <= 12,
        (2 | 3 | 8 | 10 | 13 | 16, []) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn historical_only(phase: u8) -> bool {
    matches!(phase, 4 | 12 | 14 | 15 | 17)
}
fn invoke_originals(
    phase: u8,
    handle: u64,
    originals: &[Vec<u8>],
) -> Result<KagemushaOrdinaryNativeMintFundingResponseV1, Error> {
    require_shape(
        phase,
        handle,
        &originals.iter().map(Vec::len).collect::<Vec<_>>(),
    )?;
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected);
    }
    // Proof/recovery data needs authentic installed identity + registry selection, not a live
    // 120-second account observation held across hours-long proof work. No effect uses this gate.
    if historical_only(phase) {
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else if phase == 13 {
        backend
            .source
            .recheck_installed_originals_for_refresh(&backend.path)?;
    } else {
        backend.source.recheck_originals(&backend.path)?;
    }
    drop(installed);
    let session = backend
        .source
        .native_account_session
        .as_ref()
        .ok_or(Error::Unavailable)?;
    if phase == 13 {
        {
            let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
            if owner.handle != Some(handle) {
                return Err(Error::Rejected);
            }
        }
        session.refresh_incoming_clock()?;
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
        {
            let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
            if owner.handle != Some(handle) {
                return Err(Error::Rejected);
            }
        }
        return Ok(KagemushaOrdinaryNativeMintFundingResponseV1 {
            version: 1,
            phase,
            core_handle: handle,
            fields: vec![],
        });
    }
    if historical_only(phase) {
        session.recheck_retained_account_identity()?;
    } else {
        session.recheck()?;
    }
    let mut owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(handle) {
        return Err(Error::Rejected);
    }
    let cash = owner.cash.as_mut().ok_or(Error::Unavailable)?;
    let fields = match phase {
        1 => {
            let amount = u128::from_le_bytes(
                originals[0]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            if amount == 0 {
                return Err(Error::Rejected);
            }
            cash.prepare_mint_funding_platform(amount)
                .map_err(|_| Error::Rejected)?
        }
        2 => {
            cash.fence_mint_funding_platform()
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        3 => {
            cash.retain_mint_funding_platform(&originals[0])
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        4 => {
            let proving = backend
                .source
                .bootstrap
                .as_ref()
                .and_then(|m| m.proving.as_ref())
                .ok_or(Error::Unavailable)?;
            let resolver = super::super::native_core_work::Resolver(proving.resolver.clone());
            vec![
                cash.prove_mint_funding_request(proving.profile.clone(), resolver)
                    .map_err(|_| Error::Rejected)?
                    .to_vec(),
            ]
        }
        5 => vec![
            cash.sign_mint_funding_consent(|loan| {
                session.sign_mint_consent(loan).map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            })
            .map_err(|_| Error::Rejected)?
            .to_vec(),
        ],
        6 => cash
            .prepare_mint_funding_predebit()
            .map_err(|_| Error::Rejected)?,
        7 => cash
            .fence_mint_funding_predebit()
            .map_err(|_| Error::Rejected)?,
        8 => {
            cash.retain_mint_funding_decision(
                &originals[0],
                &originals[1],
                &originals[2],
                &originals[3],
            )
            .map_err(|_| Error::Rejected)?;
            cash.retain_mint_funding_node_submission(&originals[4])
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        9 => vec![
            cash.sign_mint_funding_transaction(|loan| {
                session.sign_mint_transaction(loan).map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            })
            .map_err(|_| Error::Rejected)?
            .to_vec(),
        ],
        10 => {
            cash.with_mint_funding_transport(|loan| {
                session.submit_mint_transaction(loan).map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            })
            .map_err(|_| Error::Rejected)?;
            vec![]
        }
        11 => {
            let raw = cash
                .with_mint_funding_transport(|loan| {
                    session.read_mint_finality(loan).map_err(|_| {
                        iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                    })
                })
                .map_err(|_| Error::Rejected)?;
            match raw {
                None => vec![vec![0], vec![]],
                Some(raw) => {
                    cash.retain_mint_funding_finalized_original(&raw)
                        .map_err(|_| Error::Rejected)?;
                    vec![vec![1], raw]
                }
            }
        }
        12 => cash
            .recover_mint_funding_platform()
            .map_err(|_| Error::Rejected)?,
        14 => cash
            .recover_mint_funding_predebit_originals()
            .map_err(|_| Error::Rejected)?,
        15 => cash
            .mint_funding_platform_counter_original()
            .map_err(|_| Error::Rejected)?,
        16 => {
            cash.acknowledge_retained_mint_funding_platform()
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        17 => cash
            .recover_mint_funding_progress()
            .map_err(|_| Error::Rejected)?,
        _ => return Err(Error::Rejected),
    };
    // Historical-only return never grants current money/signing authority. All effect phases
    // recheck finite S/W/FI/PI/clock independently before and after their genuine Main loan.
    if historical_only(phase) {
        session.recheck_retained_account_identity()?;
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
    }
    require_fields(phase, &fields)?;
    Ok(KagemushaOrdinaryNativeMintFundingResponseV1 {
        version: 1,
        phase,
        core_handle: handle,
        fields,
    })
}
/// Decode sole bounded first-release transport then invoke the actual retained Native owner.
/// # Errors
/// Rejects malformed/foreign frames, unavailable real material and every failed custody check.
pub fn invoke_kagemusha_native_ordinary_mint_funding_v1(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.is_empty() || frame.len() > FRAME_MAX {
        return Err(Error::Rejected);
    }
    let request: KagemushaOrdinaryNativeMintFundingRequestV1 =
        norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(FRAME_MAX))
            .map_err(|_| Error::Rejected)?;
    if request.version != 1
        || request.originals.len() > 5
        || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
    {
        return Err(Error::Rejected);
    }
    let response = invoke_originals(request.phase, request.core_handle, &request.originals)?;
    norito::encode_canonical(&response).map_err(|_| Error::Rejected)
}
/// C entry for the same full original frame; no small coordinator limit truncates an original.
/// # Safety
/// Input must be readable for its checked length; output pointers must be nonnull/writable and
/// released by the standard bridge allocator. Output is reset before any refusal or input read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_mint_funding_v1(
    input: *const u8,
    input_len: usize,
    output: *mut *mut u8,
    output_len: *mut usize,
) -> std::ffi::c_int {
    if output.is_null() || output_len.is_null() {
        return crate::ERR_NULL_PTR;
    }
    unsafe {
        *output = std::ptr::null_mut();
        *output_len = 0;
    }
    if input.is_null() || input_len == 0 || input_len > FRAME_MAX {
        return crate::ERR_KAGEMUSHA_V1;
    }
    let frame =
        zeroize::Zeroizing::new(unsafe { std::slice::from_raw_parts(input, input_len) }.to_vec());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        invoke_kagemusha_native_ordinary_mint_funding_v1(&frame)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(Error::Unavailable)) => crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        _ => crate::ERR_KAGEMUSHA_V1,
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeMintFundingV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    phase: jni::sys::jint,
    handle: jni::sys::jlong,
    originals: jni::objects::JObjectArray<'local>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            #[cfg(any(target_os = "android", test))]
            super::super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &_class).map_err(|_| ())?;
            let phase = u8::try_from(phase).map_err(|_| ())?;
            let handle = u64::from_ne_bytes(handle.to_ne_bytes());
            let count = usize::try_from(env.get_array_length(&originals).map_err(|_| ())?)
                .map_err(|_| ())?;
            if count > 5 {
                return Err(());
            }
            let mut arrays = Vec::with_capacity(count);
            let mut lengths = Vec::with_capacity(count);
            for n in 0..count {
                let a: jni::objects::JByteArray<'_> = env
                    .get_object_array_element(&originals, n as i32)
                    .map_err(|_| ())?
                    .into();
                lengths.push(
                    usize::try_from(env.get_array_length(&a).map_err(|_| ())?).map_err(|_| ())?,
                );
                arrays.push(a);
            }
            require_shape(phase, handle, &lengths).map_err(|_| ())?;
            let mut raw = Vec::with_capacity(count);
            for (a, n) in arrays.iter().zip(lengths) {
                let bytes = env.convert_byte_array(a).map_err(|_| ())?;
                if bytes.len() != n {
                    return Err(());
                }
                raw.push(bytes);
            }
            let raw = zeroize::Zeroizing::new(raw);
            let response = invoke_originals(phase, handle, &raw).map_err(|_| ())?;
            let mut fields = vec![
                response.version.to_le_bytes().to_vec(),
                vec![response.phase],
                response.core_handle.to_le_bytes().to_vec(),
            ];
            fields.extend(response.fields);
            let output = env
                .new_object_array(fields.len() as i32, "[B", jni::objects::JObject::null())
                .map_err(|_| ())?;
            for (n, field) in fields.iter().enumerate() {
                let a = env.byte_array_from_slice(field).map_err(|_| ())?;
                env.set_object_array_element(&output, n as i32, &a)
                    .map_err(|_| ())?;
            }
            #[cfg(any(target_os = "android", test))]
            super::super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &_class).map_err(|_| ())?;
            Ok(output.into_raw())
        },
    ));
    match result {
        Ok(Ok(output)) => output,
        _ => std::ptr::null_mut(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn funding_frames_require_all_five_originals_and_positive_amount_role() {
        assert!(require_shape(1, 1, &[16]).is_ok());
        assert!(require_shape(1, 1, &[15]).is_err());
        assert!(
            require_shape(
                8,
                1,
                &[DECISION_MAX, CLOCK_MAX, CONTROL_MAX, DATA_MAX, NODE_MAX]
            )
            .is_ok()
        );
        assert!(require_shape(8, 1, &[1, 1, 1, 1]).is_err());
        assert!(require_shape(8, 1, &[1, 1, 1, 1, NODE_MAX + 1]).is_err());
        assert!(require_shape(4, 0, &[]).is_err());
        assert!(require_shape(4, 1, &[32]).is_err());
        assert!(!historical_only(5));
        assert!(!historical_only(9));
        assert!(!historical_only(10));
    }
    #[test]
    fn funding_progress_is_an_exact_read_only_operation_stage_projection() {
        assert!(require_shape(17, 1, &[]).is_ok());
        assert!(require_shape(17, 1, &[1]).is_err());
        assert!(require_shape(17, 0, &[]).is_err());
        assert!(historical_only(17));
        for stage in 0..=12 {
            assert!(require_fields(17, &[vec![1; 32], vec![stage]]).is_ok());
        }
        assert!(require_fields(17, &[vec![1; 32], vec![13]]).is_err());
        assert!(require_fields(17, &[vec![0; 32], vec![0]]).is_err());
        assert!(require_fields(17, &[vec![1; 32], vec![]]).is_err());
        assert!(require_fields(17, &[vec![1; 32], vec![0, 1]]).is_err());
        assert!(require_fields(17, &[vec![1; 32], vec![0], vec![1]]).is_err());
    }
    #[test]
    fn funding_pending_read_and_unknown_key_original_never_claim_ack() {
        assert!(require_fields(11, &[vec![0], vec![]]).is_ok());
        assert!(require_fields(11, &[vec![0], vec![1]]).is_err());
        assert!(require_fields(11, &[vec![1], vec![]]).is_err());
        let row = |status, raw| vec![vec![status], vec![1; 32], vec![2], vec![3], raw];
        assert!(require_fields(12, &row(1, vec![])).is_ok());
        assert!(require_fields(12, &row(1, vec![4])).is_err());
        assert!(require_fields(12, &row(3, vec![])).is_err());
        assert!(require_fields(12, &row(3, vec![4])).is_ok());
    }
}
