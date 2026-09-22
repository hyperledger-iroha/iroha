import { test } from "node:test";
import { SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1, SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS, validateFixtureBundle } from "../src/sorafs.js";
import { nativeBinding, nativeBindingError } from "./helpers/native.js";
import { registerSorafsFixtureBundleValidationTests } from "./sorafsNativeSuites/sorafsFixtureBundleValidation.js";

registerSorafsFixtureBundleValidationTests({
  test,
  subject: { SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1, SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS, validateFixtureBundle },
  fixtureRoot: new URL("../../../fixtures/sorafs_manifest/", import.meta.url),
  nativeBinding,
  nativeBindingError,
});
