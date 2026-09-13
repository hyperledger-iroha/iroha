import type { SorafsPinManifestSummaryV1, SorafsReplicationStatus, SorafsReplicationOrderRecord, SorafsReplicationListOptions } from "../../../index.js";
const approved: SorafsPinManifestSummaryV1 = {
  digest: new Uint8Array(32), submitted_by: "account", submitted_epoch: 1n,
  approved_epoch: 2n, content_length: 3n, retention_epoch: 4n,
  status: { status: "Approved", value: 2n }, successor_of: null,
};
const cancelled: SorafsReplicationStatus = { state: "cancelled", epoch: 18446744073709551615n };
const pending: SorafsReplicationStatus = { state: "pending" };
const emptyCompletions: Pick<SorafsReplicationOrderRecord, "provider_completions" | "assignment_revision"> = { provider_completions: [], assignment_revision: 1n };
// @ts-expect-error approved_epoch is a required nullable field in every native summary.
const missingApproval: SorafsPinManifestSummaryV1 = { digest: new Uint8Array(32), submitted_by: "account", submitted_epoch: 1, content_length: 1, retention_epoch: 4, status: { status: "Pending", value: null }, successor_of: null };
// @ts-expect-error pending carries no terminal epoch.
const pendingEpoch: SorafsReplicationStatus = { state: "pending", epoch: null };
// @ts-expect-error completed carries a required exact epoch.
const completedNoEpoch: SorafsReplicationStatus = { state: "completed" };
// @ts-expect-error terminal epochs never accept string aliases.
const stringEpoch: SorafsReplicationStatus = { state: "expired", epoch: "3" };
// @ts-expect-error retired receipt records are not an inventory field.
const receipts: Pick<SorafsReplicationOrderRecord, "receipts"> = { receipts: [] };
// @ts-expect-error status filter spellings are exact.
const uppercase: Pick<SorafsReplicationListOptions, "status"> = { status: "Cancelled" };
// @ts-expect-error native pagination inputs use bounded integers.
const stringLimit: Pick<SorafsReplicationListOptions, "limit"> = { limit: "5" };
void [approved, cancelled, pending, emptyCompletions, missingApproval, pendingEpoch, completedNoEpoch, stringEpoch, receipts, uppercase, stringLimit];
