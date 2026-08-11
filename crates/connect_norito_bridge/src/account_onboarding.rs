//! Sponsored account-onboarding plan encoding for the Connect C ABI.

use std::slice;

use iroha_data_model::{NetworkId, account::AccountId};
use libc::{c_int, c_uchar, c_ulong};

use super::{
    BridgeError, DETACHED_TRANSACTION_JSON_MAX_BYTES, bridge_result_to_code, clear_bridge_output,
    write_bytes_bridge,
};

#[derive(Debug, norito::Encode, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ConnectAccountOnboardingPlanRequestV1 {
    pub(crate) version: u8,
    pub(crate) alias: String,
    pub(crate) account_id: String,
    pub(crate) permissions: Vec<String>,
}

#[derive(Debug, norito::Encode, norito::JsonSerialize, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct ConnectAccountOnboardingPlanBodyV1 {
    pub(crate) version: u8,
    pub(crate) request: ConnectAccountOnboardingPlanRequestV1,
    pub(crate) authority: AccountId,
    pub(crate) network_id: NetworkId,
    pub(crate) anchor: iroha_data_model::alias_setup::AliasPlanAnchorV1,
    pub(crate) resource: iroha_data_model::alias_setup::AliasPlanResourceV1,
    pub(crate) acquisition: iroha_data_model::alias_setup::AliasLeaseAcquisitionV1,
    pub(crate) quote_guard: iroha_data_model::alias_setup::AliasQuoteGuardV1,
    pub(crate) instructions: Vec<iroha_data_model::alias_setup::AliasFramedInstructionV1>,
    #[norito(default)]
    pub(crate) owner_auto_renew_instruction:
        Option<iroha_data_model::alias_setup::AliasFramedInstructionV1>,
    pub(crate) valid_until_ms: u64,
}

/// Encode an exact sponsored-onboarding plan body from its typed JSON form.
///
/// The returned buffer is the bare canonical Norito encoding committed by the
/// V1 receipt hash. Callers release it with [`crate::connect_norito_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_account_onboarding_plan_body_v1(
    json_ptr: *const c_uchar,
    json_len: c_ulong,
    out_body_ptr: *mut *mut c_uchar,
    out_body_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_body_ptr, out_body_len);
    let result = (|| {
        if out_body_ptr.is_null() || out_body_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let json_len = usize::try_from(json_len).map_err(|_| BridgeError::AccountOnboardingBody)?;
        if json_len == 0 || json_len > DETACHED_TRANSACTION_JSON_MAX_BYTES || json_ptr.is_null() {
            return Err(BridgeError::AccountOnboardingBody);
        }
        let input = unsafe { slice::from_raw_parts(json_ptr, json_len) };
        let body: ConnectAccountOnboardingPlanBodyV1 =
            norito::json::from_slice(input).map_err(|_| BridgeError::AccountOnboardingBody)?;
        if body.version != 1 || body.request.version != 1 {
            return Err(BridgeError::AccountOnboardingBody);
        }
        use norito::codec::Encode as _;
        let encoded = body.encode();
        if encoded.is_empty() {
            return Err(BridgeError::AccountOnboardingBody);
        }
        unsafe { write_bytes_bridge(out_body_ptr, out_body_len, &encoded) }
    })();
    bridge_result_to_code(result)
}
