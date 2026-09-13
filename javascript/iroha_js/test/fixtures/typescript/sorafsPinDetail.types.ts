import type {
  SorafsPinManifestResponse,
  SorafsManifestRecord,
  SorafsPinManifestReadOptions,
  SorafsPinNativeStatus,
  ToriiClientOptions,
} from "../../../index.js";

const anchor: SorafsPinManifestReadOptions = {
  expectedFinalizedHeight: 18446744073709551615n,
  expectedFinalizedBlockHashHex: "42".repeat(32),
};
const manifest: SorafsManifestRecord = {
  digest: new Uint8Array(32), root_cid: new Uint8Array(36),
  chunker: { profile_id: 1, namespace: "sorafs", name: "sf1", semver: "1.0.0", multihash_code: 31n },
  chunk_digest_sha3_256: new Uint8Array(32), por_root: new Uint8Array(32),
  content_length: 18446744073709551615n,
  policy: { min_replicas: 3, storage_class: { type: "Hot", value: null }, retention_epoch: 100n },
  submitted_by: "canonical-owner", submitted_epoch: 42, approved_epoch: 45n,
  alias: { namespace: "docs", name: "main", proof: "YQ==" },
  metadata: { fractional: 1.25 }, status: { status: "Approved", value: 45n },
  council_envelope_digest: null,
};
const response: SorafsPinManifestResponse = { finalized_cursor: { height: 51n, block_hash: new Uint8Array(32) }, manifest };
const retiredHeight: SorafsPinManifestReadOptions = {
  // @ts-expect-error schema integers never accept a string token alias.
  expectedFinalizedHeight: "51",
};
const retiredEnvelope: SorafsPinManifestResponse = {
  ...response,
  // @ts-expect-error aliases are returned through their dedicated route.
  aliases: [],
};
const retiredStatus: SorafsPinNativeStatus = {
  // @ts-expect-error native status union retains exact PascalCase tags.
  status: "approved", value: 45,
};
const retiredNull: Pick<SorafsManifestRecord, "successor_of"> = {
  // @ts-expect-error absent optional native fields are omitted, never null.
  successor_of: null,
};
const retiredPolicy: ToriiClientOptions = {
  // @ts-expect-error the finalized pin reader has no alias-header policy facade.
  sorafsAliasPolicy: {},
};
void [anchor, response, retiredHeight, retiredEnvelope, retiredStatus, retiredNull, retiredPolicy];
