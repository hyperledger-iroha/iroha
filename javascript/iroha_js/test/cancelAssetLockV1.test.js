import test from "node:test";
import { decodeCancelAssetLockV1, encodeCancelAssetLockV1 } from "../src/norito.js";
import { registerCancelAssetLockV1Tests } from "./sorafsNativeSuites/cancelAssetLockV1.js";

registerCancelAssetLockV1Tests({
  test,
  subject: { decodeCancelAssetLockV1, encodeCancelAssetLockV1 },
  fixtureRoot: new URL("../../../fixtures/sorafs_manifest/appeal_finance/", import.meta.url),
});
