import type {
  AppApiTransactionDraft,
  NumericLike,
  QuantityInput,
} from "../index.js";

export type SubscriptionPlan = Record<string, unknown>;
export type SubscriptionState = Record<string, unknown>;
export type SubscriptionInvoice = Record<string, unknown>;

export interface SubscriptionPlanCreateRequest {
  authority: string;
  planId: string;
  plan: SubscriptionPlan;
}

export interface SubscriptionPlanCreateResponse extends AppApiTransactionDraft {
  plan_id: string;
}

export interface SubscriptionPlanListItem {
  plan_id: string;
  plan: SubscriptionPlan;
}

export interface SubscriptionPlanListResponse {
  items: ReadonlyArray<SubscriptionPlanListItem>;
  total: number;
}

export interface SubscriptionCreateRequest {
  authority: string;
  subscriptionId: string;
  planId: string;
  billingTriggerId?: string;
  usageTriggerId?: string | null;
  firstChargeMs?: NumericLike;
  grantUsageToProvider?: boolean;
}

export interface SubscriptionMutationInstructionDraft {
  wire_id: string;
  payload_hex: string;
}

export interface SubscriptionCreateResponse {
  version: 1;
  authority: string;
  action: "create";
  subscription_id: string;
  plan_id: string;
  billing_trigger_id: string;
  usage_trigger_id: string | null;
  first_charge_ms: number;
  provider_usage_grant_included: boolean;
  resulting_subscription: SubscriptionState;
  tx_instructions: ReadonlyArray<SubscriptionMutationInstructionDraft>;
}

export interface SubscriptionListItem {
  subscription_id: string;
  subscription: SubscriptionState;
  invoice?: SubscriptionInvoice | null;
  plan?: SubscriptionPlan | null;
}

export interface SubscriptionListResponse {
  items: ReadonlyArray<SubscriptionListItem>;
  total: number;
}

export interface SubscriptionGetResponse {
  subscription_id: string;
  subscription: SubscriptionState;
  invoice?: SubscriptionInvoice | null;
  plan?: SubscriptionPlan | null;
}

export interface SubscriptionAuthorityActionRequest {
  authority: string;
}

export interface SubscriptionChargeActionRequest
  extends SubscriptionAuthorityActionRequest {
  chargeAtMs?: NumericLike;
}

export interface SubscriptionCancelActionRequest
  extends SubscriptionAuthorityActionRequest {
  cancelMode: "immediate" | "period_end";
}

export type SubscriptionActionRequest =
  | SubscriptionAuthorityActionRequest
  | SubscriptionChargeActionRequest
  | SubscriptionCancelActionRequest;

export interface SubscriptionUsageRequest {
  authority: string;
  unitKey: string;
  delta: QuantityInput;
  usageTriggerId?: string | null;
}

export interface SubscriptionUsageDraft extends AppApiTransactionDraft {
  subscription_id: string;
}

export interface SubscriptionCancelModeV1 {
  mode: "immediate" | "period_end";
  value: null;
}

export interface SubscriptionActionDraftDetails {
  billing_trigger_id: string;
  billing_trigger_operation: "none" | "register" | "unregister" | "replace";
  effective_charge_ms: number | null;
  cancel_mode: SubscriptionCancelModeV1 | null;
  resulting_subscription: SubscriptionState;
}

export interface SubscriptionActionResponse {
  version: 1;
  authority: string;
  action: "pause" | "resume" | "cancel" | "keep" | "charge_now";
  subscription_id: string;
  details: SubscriptionActionDraftDetails;
  tx_instructions: ReadonlyArray<SubscriptionMutationInstructionDraft>;
}
