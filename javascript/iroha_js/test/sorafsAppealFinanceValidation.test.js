import test from "node:test";
import { validateAppealFinanceCancelAssetLock } from "../src/sorafs.js";
import { registerSorafsAppealFinanceValidationTests } from "./sorafsNativeSuites/sorafsAppealFinanceValidation.js";

registerSorafsAppealFinanceValidationTests({
  test,
  subject: { validateAppealFinanceCancelAssetLock },
  fixtureRoot: new URL("../../../fixtures/sorafs_manifest/appeal_finance/", import.meta.url),
});
