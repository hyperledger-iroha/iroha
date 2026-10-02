import { computeValidationFeePolicyProposalFingerprintV1 } from "../../../index.js";
import type {
  JsonValue, RetailFeeAssessmentV1, RetailFeeQuoteRequestV1, RetailFeeScheduleV1,
  ToriiGovernanceValidationFeeChargingMode, ToriiGovernanceValidationFeePolicyV1,
  ToriiGovernanceValidationFeePayoutBinding,
} from "../../../index.js";
const enabled: ToriiGovernanceValidationFeeChargingMode = {
  charging_mode: "RETAIL_MONTHLY_ALLOWANCE", value: null,
};
const disabled: ToriiGovernanceValidationFeeChargingMode = {
  // @ts-expect-error the first release has no disabled charging mode
  charging_mode: "DISABLED", value: null,
};
declare const policy: ToriiGovernanceValidationFeePolicyV1;
const exemption: readonly ["TREASURY_PAYOUT"] = policy.exemption_classes;
const effective: number | bigint = policy.effective_from_ms;
const { exemption_classes: omitted, ...missingExemption } = policy;
// @ts-expect-error the exact policy shape requires the sole exemption class
const incomplete: ToriiGovernanceValidationFeePolicyV1 = missingExemption;
declare const assessment: RetailFeeAssessmentV1;
const exactAssessment: RetailFeeAssessmentV1 = { ...assessment, fee_minor: 0n };
const invalidAssessment: RetailFeeAssessmentV1 = { ...assessment,
  // @ts-expect-error the real assessment normalizer refuses decimal string aliases
  policy_revision: "1",
};
declare const schedule: RetailFeeScheduleV1;
const exactSchedule: RetailFeeScheduleV1 = { ...schedule, overage_minor: 10n };
const invalidSchedule: RetailFeeScheduleV1 = { ...schedule,
  // @ts-expect-error the exact policy parser refuses string integer aliases
  overage_minor: "10",
};
declare const payout: ToriiGovernanceValidationFeePayoutBinding;
const exactPayout: ToriiGovernanceValidationFeePayoutBinding = {
  ...payout, max_sbd_per_attempt_minor: 100n,
};
const invalidPayout: ToriiGovernanceValidationFeePayoutBinding = { ...payout,
  // @ts-expect-error the actual conversion parser accepts only exact number/bigint
  max_sbd_per_attempt_minor: "100",
};
const quote: RetailFeeQuoteRequestV1 = {
  account_id: "canonical-account", asset_definition_id: "canonical-asset",
  transfers: [{ destination_account_id: "canonical-receiver", amount_minor_units: "10" }],
};
void [enabled, disabled, exemption, effective, omitted, incomplete, exactAssessment,
  invalidAssessment, exactSchedule, invalidSchedule, exactPayout, invalidPayout, quote];

declare const canonicalPolicy: Readonly<Record<string, JsonValue>>;
const fingerprint: string = computeValidationFeePolicyProposalFingerprintV1("canonical-operator", canonicalPolicy);
// @ts-expect-error pricing does not accept a retired conversion proposal ID argument
computeValidationFeePolicyProposalFingerprintV1("canonical-operator", canonicalPolicy, "retired-lifecycle-id");
void fingerprint;
