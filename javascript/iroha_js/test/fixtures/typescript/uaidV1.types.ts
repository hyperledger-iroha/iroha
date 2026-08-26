import type {
  UaidAssetPermissionManifest,
  UaidManifestQueryOptions,
  UaidManifestsResponse,
} from "../../../index.js";

const uaid = `uaid:${"01".repeat(31)}03`;

const manifest: UaidAssetPermissionManifest = {
  version: 1,
  uaid,
  dataspace: 11,
  issued_ms: 1,
  activation_epoch: 2,
  entries: [
    {
      scope: { dataspace: 11, program: "cbdc.transfer" },
      effect: { Allow: { window: "PerDay", max_amount: "500" } },
    },
  ],
};

const legacyVersion: UaidAssetPermissionManifest = {
  // @ts-expect-error V1 manifest JSON uses the exact numeric version.
  version: "V1",
  uaid,
  dataspace: 11,
  issued_ms: 1,
  activation_epoch: 2,
  entries: [],
};

const query: UaidManifestQueryOptions = {
  dataspaceId: 11,
  status: "active",
  limit: 10,
  offset: 0,
  countMode: "exact",
};

const legacyQuery: UaidManifestQueryOptions = {
  // @ts-expect-error snake-case aliases are not part of the JS V1 surface.
  count_mode: "exact",
};

const invalidStatus: UaidManifestQueryOptions = {
  // @ts-expect-error manifest filters are exact lower-case V1 labels.
  status: "Active",
};

const response: UaidManifestsResponse = {
  uaid,
  total: 0,
  has_more: false,
  count_mode: "exact",
  manifests: [],
};

// @ts-expect-error page metadata is mandatory in the current response.
const legacyResponse: UaidManifestsResponse = { uaid, manifests: [] };

void manifest;
void legacyVersion;
void query;
void legacyQuery;
void invalidStatus;
void response;
void legacyResponse;
