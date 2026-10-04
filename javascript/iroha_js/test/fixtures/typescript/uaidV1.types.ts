import type {
  UaidAssetPermissionManifest,
  ListQueryInput,
  Page,
  UaidManifestRecord,
  ToriiClient,
  ToriiBrowserClient,
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

const query: ListQueryInput = {
  filter: 'dataspace_id = 11 and status = "active"',
  limit: 10,
  includeTotal: true,
};

const legacyQuery: ListQueryInput = {
  // @ts-expect-error offset paging is retired.
  offset: 0,
};

const invalidStatus: ListQueryInput = {
  // @ts-expect-error endpoint filters belong in the shared filter expression.
  status: "active",
};

const response: Page<UaidManifestRecord> = {
  total: 0,
  nextCursor: null,
  items: [],
};

// @ts-expect-error nextCursor is mandatory in the current response.
const legacyResponse: Page<UaidManifestRecord> = { items: [] };

void manifest;
void legacyVersion;
void query;
void legacyQuery;
void invalidStatus;
void response;
void legacyResponse;

function collectionTypes(client: ToriiClient) {
  void client.accountHistory("alice@wonderland").list({ filter: "block_height >= 10" });
  void client.contractEvents.list({ select: ["block_index"] });
  void client.explorerLatestInstructions.list({ limit: 25 });
  void client.subscriptionPlans.list({ includeTotal: true });
  // @ts-expect-error bounded Explorer feeds have fixed server order.
  void client.explorerNfts.list({ sort: "id" });
  // @ts-expect-error account movements have no exact total.
  void client.accountHistory("alice@wonderland").list({ includeTotal: true });
  // @ts-expect-error old list wrappers are absent from the first-release surface.
  void client.listExplorerNfts();
}
void collectionTypes;

function browserCollectionTypes(client: ToriiBrowserClient) {
  void client.accountHistory("alice@wonderland").list({ filter: "block_height >= 10" });
  void client.contractEvents.list({ select: ["block_index"] });
  void client.explorerLatestInstructions.list({ limit: 25 });
  void client.subscriptionPlans.list({ includeTotal: true });
  // @ts-expect-error bounded Explorer feeds have fixed server order.
  void client.explorerNfts.list({ sort: "id" });
}
void browserCollectionTypes;
