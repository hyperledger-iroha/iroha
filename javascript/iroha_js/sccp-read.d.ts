/** Exact native JSON for the current public SCCP metadata routes. These records do not verify proofs. */
export type SccpUnsignedIntegerV1 = number | bigint;
/** Fixed-size native byte arrays are uppercase hex text without a prefix. */
export type SccpHexV1 = string;
export type SccpExternalNetworkKeyV1 = "ethereum-mainnet" | "bsc-mainnet" | "ton-mainnet";
export interface SccpExternalNetworkV1 {
  network: "ethereum_mainnet" | "bsc_mainnet" | "ton_mainnet";
  profile: null;
}
export interface SccpPublicReadOptionsV1 { signal?: AbortSignal }
export interface SccpRecentMessagesOptionsV1 extends SccpPublicReadOptionsV1 {
  /** Defaults to outbound. */
  direction?: "outbound" | "inbound";
  network?: SccpExternalNetworkKeyV1;
  /** Native next_before cursor; inbound message-id suffixes use lowercase hex. */
  before?: string;
  /** Integer in 1..50; defaults to 50. */
  limit?: number;
}
export interface SccpParametersV1 {
  enabled: boolean;
  committee_validity_ms: SccpUnsignedIntegerV1;
  committee_heartbeat_ms: SccpUnsignedIntegerV1;
  forgery_evidence_grace_ms: SccpUnsignedIntegerV1;
  destination_progress_cap_ms: SccpUnsignedIntegerV1;
  outbound_ttl_ms: SccpUnsignedIntegerV1;
  min_outbound_amount: string;
  inbound_self_claim_fee: string;
  max_exempt_transactions_per_block: number;
  fast_pause_panel_seats: number;
  fast_pause_panel_term_ms: SccpUnsignedIntegerV1;
  fast_pause_seat_accept_window_ms: SccpUnsignedIntegerV1;
  fast_pause_endorse_window_ms: SccpUnsignedIntegerV1;
  fast_pause_renew_lead_ms: SccpUnsignedIntegerV1;
  fast_pause_hold_ms: SccpUnsignedIntegerV1;
}
export interface SccpCapabilitiesV1 {
  network_id: SccpHexV1;
  consensus_instance: SccpHexV1;
  finality: { x_layout_id: SccpHexV1; dst_sig: SccpHexV1; result_tag: SccpHexV1; result_body_tag: SccpHexV1 };
  current_generation: {
    generation: SccpUnsignedIntegerV1;
    committee_root: SccpHexV1;
    start_height: SccpUnsignedIntegerV1;
    deadline_ms: SccpUnsignedIntegerV1;
  } | null;
  coalition_slashable_floor: string | null;
  liability_clock: SccpUnsignedIntegerV1;
  forgery_max_generation: SccpUnsignedIntegerV1 | null;
  pause_panel_ids: { primary: SccpHexV1; backup: SccpHexV1 } | null;
  parliament_binding_ballot_available: boolean;
  reconciliation_available: boolean;
  parameters: SccpParametersV1 | null;
  committed_height: SccpUnsignedIntegerV1;
  light_client_profiles: Array<{
    network: SccpExternalNetworkV1;
    version: number;
    profile_hash: SccpHexV1;
    activation_height: SccpUnsignedIntegerV1 | null;
    proposal_id: SccpHexV1 | null;
    supported_until_ms: SccpUnsignedIntegerV1 | null;
  }>;
  chain_id: string;
  profiles: Array<{ network: SccpExternalNetworkV1; supported_until_ms: SccpUnsignedIntegerV1 | null }>;
  profiles_policy_hash: SccpHexV1;
  limits: { recent_messages: number; outbound_page: number; controls_page: number; rotations_page: number };
  path_templates: string[];
}
export interface SccpCommitteePinV1 {
  generation: SccpUnsignedIntegerV1;
  committee_root: SccpHexV1;
  start_height: SccpUnsignedIntegerV1;
  until_ms: SccpUnsignedIntegerV1;
}
export interface SccpTonCodeRefV1 { hash: SccpHexV1; depth: number }
export type SccpDeploymentV1 =
  | { family: "evm"; deployment: { address: SccpHexV1; runtime_code_hash: SccpHexV1 } }
  | { family: "ton"; deployment: {
    master_account: SccpHexV1;
    minter_code: SccpTonCodeRefV1;
    wallet_code: SccpTonCodeRefV1;
    bucket_code: SccpTonCodeRefV1;
  } };
export interface SccpRouteRevisionV1 {
  revision: number;
  deployment: SccpDeploymentV1;
  destination_word: SccpHexV1;
  initial_committee: SccpCommitteePinV1;
  activation: { activation: "staged" | "bidirectional" | "paused" | "inbound_only" | "retired"; detail: null };
  destination_frozen: boolean;
  destination_paused: boolean;
  next_control_nonce: SccpUnsignedIntegerV1;
  max_wrapped_supply: string;
  liability: string;
  next_outbound_nonce: SccpUnsignedIntegerV1;
  registered_at_height: SccpUnsignedIntegerV1;
  forgery_floor: SccpUnsignedIntegerV1;
  quarantine: { certificate_id: SccpHexV1; since_height: SccpUnsignedIntegerV1 } | null;
  ever_activated: boolean;
}
export interface SccpRouteV1 {
  network: SccpExternalNetworkV1;
  route_id: "taira_eth_xor" | "taira_bsc_xor" | "taira_ton_xor";
  /** Canonical I105 account literal. */
  escrow: string;
  stranded: string;
  revisions: Record<string, SccpRouteRevisionV1>;
}
export interface SccpPendingReasonV1 {
  reason: "disabled" | "revision_not_settleable" | "liability_shortfall" | "credit_refused" |
    "fee_sink_unavailable" | "block_leaves_full" | "held" | "refund_deferred";
  detail: null;
}
export interface SccpVoidStatusV1 {
  kind: { kind: "expired" | "frozen"; detail: null };
  proven_at_height: SccpUnsignedIntegerV1;
  refund_pending: SccpPendingReasonV1 | null;
}
export type SccpOutboundStatusV1 =
  | { status: "recorded"; detail: null }
  | { status: "voided"; detail: SccpVoidStatusV1 }
  | { status: "refunded" | "stranded"; detail: { height: SccpUnsignedIntegerV1 } };
export type SccpOutboundStateV1 =
  | { state: "recorded"; detail: { deadline_ms: SccpUnsignedIntegerV1; provable: boolean } }
  | { state: "voided"; detail: SccpVoidStatusV1 }
  | { state: "refunded" | "stranded"; detail: { height: SccpUnsignedIntegerV1 } };
export interface SccpOutboundMessageRecordV1 {
  network: SccpExternalNetworkV1;
  revision: number;
  nonce: SccpUnsignedIntegerV1;
  height: SccpUnsignedIntegerV1;
  commitment_index: number;
  deadline_ms: SccpUnsignedIntegerV1;
  sender: string;
  amount: string;
  /** Canonical padded base64, at most 4096 decoded bytes. */
  payload: string;
  leaf: SccpHexV1;
  status: SccpOutboundStatusV1;
}
export type SccpInboundStatusV1 =
  | { status: "pending"; detail: { reason: SccpPendingReasonV1 } }
  | { status: "released" | "stranded"; detail: { height: SccpUnsignedIntegerV1; payout: SccpUnsignedIntegerV1 } }
  | { status: "bounced"; detail: { bounce_message_id: SccpHexV1; payout: SccpUnsignedIntegerV1 } };
export interface SccpInboundRecordV1 {
  network: SccpExternalNetworkV1;
  revision: number;
  payload: string;
  source_locator: { source_height: SccpUnsignedIntegerV1; block_hash: SccpHexV1; index_in_block: number };
  proven_at_height: SccpUnsignedIntegerV1;
  fee_due: string;
  status: SccpInboundStatusV1;
}
export type SccpRecentMessageV1 =
  | { direction: "outbound"; detail: { message_id: SccpHexV1; record: SccpOutboundMessageRecordV1; state: SccpOutboundStateV1 } }
  | { direction: "inbound"; detail: { message_id: SccpHexV1; record: SccpInboundRecordV1 } };
export interface SccpRecentMessagesV1 {
  messages: SccpRecentMessageV1[];
  /** May be non-null even on an empty page when the native scan budget ends. */
  next_before: string | null;
}
