//! The only two native purpose adapters; their public record schemas remain distinct.
use super::*;
use iroha_data_model::sorafs::{
    final_promotion_account_custody::{
        self as account, FinalPromotionAccountCustodyExecutionV1,
        FinalPromotionAccountCustodyRecordV1,
    },
    final_promotion_authority::{
        self as receipt, FinalPromotionCustodyRecordV1, FinalPromotionExecutionV1,
    },
};

pub(crate) struct ReceiptPurpose;
pub(crate) struct AccountPurpose;

macro_rules! purpose {
    ($purpose:ident, $record:ident, $execution:ident, $namespace:literal, $domain:path,
     $role:ident, $maximum:path, $normal:path, $bound:path) => {
        impl sealed::Purpose for $purpose {}
        impl sealed::Execution for $execution {}
        impl CustodyExecution for $execution {
            fn view(&self) -> ExecutionView<'_> {
                ExecutionView {
                    height: self.height,
                    ordinal: self.ordinal,
                    recorded_at_unix_ms: self.recorded_at_unix_ms,
                    authority: &self.authority,
                }
            }
            fn build(value: ExecutionView<'_>) -> Self {
                Self {
                    height: value.height,
                    ordinal: value.ordinal,
                    recorded_at_unix_ms: value.recorded_at_unix_ms,
                    authority: value.authority.clone(),
                }
            }
        }
        impl CustodyPurpose for $purpose {
            type Record = $record;
            type Execution = $execution;
            const NAMESPACE: &'static str = $namespace;
            const RECORD_DOMAIN: &'static [u8] = $domain;
            const ROLE: SignerRoleV1 = SignerRoleV1::$role;
            const MAX_REVISIONS: u64 = $maximum;
            const NORMAL_REVISIONS: u64 = $normal;
            fn purpose(deployment_id: String) -> SignerPurposeBindingV1 {
                SignerPurposeBindingV1::$role { deployment_id }
            }
            fn record_view(record: &Self::Record) -> ControlRecordView<'_, Self::Execution> {
                ControlRecordView {
                    deployment: &record.deployment_id,
                    revision: record.revision,
                    predecessor_digest: record.predecessor_digest,
                    request_digest: record.request_digest,
                    execution: &record.execution,
                    control_state: &record.control_state,
                    enrollment: record.enrollment.as_deref(),
                }
            }
            fn build_record(parts: ControlRecordParts<Self::Execution>) -> Self::Record {
                $record {
                    deployment_id: parts.deployment,
                    revision: parts.revision,
                    predecessor_digest: parts.predecessor_digest,
                    request_digest: parts.request_digest,
                    execution: parts.execution,
                    control_state: parts.control_state,
                    enrollment: parts.enrollment,
                }
            }
        }
        const _: () = assert!($bound == MAX_FRAME_BYTES_V1);
    };
}
purpose!(
    ReceiptPurpose,
    FinalPromotionCustodyRecordV1,
    FinalPromotionExecutionV1,
    "sorafs_final_promotion_authority_v1",
    receipt::FINAL_PROMOTION_CUSTODY_RECORD_DOMAIN_V1,
    FinalPromotionProvenance,
    receipt::FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1,
    receipt::FINAL_PROMOTION_CUSTODY_NORMAL_REVISIONS_V1,
    receipt::FINAL_PROMOTION_MAX_RECORD_BYTES_V1
);
purpose!(
    AccountPurpose,
    FinalPromotionAccountCustodyRecordV1,
    FinalPromotionAccountCustodyExecutionV1,
    "sorafs_final_promotion_account_custody_v1",
    account::FINAL_PROMOTION_ACCOUNT_CUSTODY_RECORD_DOMAIN_V1,
    FinalPromotionAccountTransaction,
    account::FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1,
    account::FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1,
    account::FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1
);
