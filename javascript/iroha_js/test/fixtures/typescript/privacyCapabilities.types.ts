import { NetworkId, type PrivacyExact12CapabilityAdmissionV1 as RootAdmission } from "../../../index.js";
import {
  type PrivacyExact12CapabilityManifestV1,
  type PrivacyExact12CapabilityAdmissionV1,
  type PrivacyCapabilityReadinessV1,
  type PrivacyCompiledProfileResultV1,
} from "../../../privacy-capabilities.js";

declare const manifest: PrivacyExact12CapabilityManifestV1;
declare const admission: PrivacyExact12CapabilityAdmissionV1;
const rootAdmission: RootAdmission = admission;
const network: NetworkId = manifest.network_id;
const available: "available" = admission.readiness;
const readiness: PrivacyCapabilityReadinessV1 = { readiness: "available", detail: null };
const unsupported: PrivacyCompiledProfileResultV1 = {
  status: "unavailable",
  value: { reason: "joint-security-reduction-incomplete", detail: null },
};
void [rootAdmission, network, available, readiness, unsupported];
// @ts-expect-error Runtime manifests no longer embed global release receipts.
manifest.qualification;
// @ts-expect-error NetworkId is a typed exact identity, not a byte array projection.
const rawNetwork: Uint8Array = manifest.network_id;
// @ts-expect-error Manifest network identity is immutable.
manifest.network_id = network;
// @ts-expect-error Retired documentary readiness is not an alias.
const retired: PrivacyCapabilityReadinessV1 = { readiness: "production-qualified", detail: null };
// @ts-expect-error Runtime qualification projection types are retired.
type OldSubpath = import("../../../privacy-capabilities.js").PrivacyExact12QualificationRecordV1;
// @ts-expect-error Root exports also remove the retired projection.
type OldRoot = import("../../../index.js").PrivacyExact12QualificationRecordV1;

const policy = manifest.protocols[0].policy;
const pending = manifest.consensus_policy.pending_update;
void [policy, pending];
// @ts-expect-error Activation records are retired, not optional aliases.
manifest.protocols[0].activation;
// @ts-expect-error Tightening-only policy is retired.
manifest.consensus_policy.pending_tightening;
// @ts-expect-error Lifecycle does not determine implementation availability.
const retiredReason: PrivacyCapabilityReadinessV1 = { readiness: "unavailable", detail: { reason: "not-registered", detail: null } };
// @ts-expect-error Retired activation types are not exported.
type OldActivation = import("../../../privacy-capabilities.js").PrivacyProtocolActivationRecordV1;
