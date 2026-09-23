import { test } from "node:test";
import { LocalSigningContext, SorafsOrderbookSubmissionAmbiguousError, ToriiClient } from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";
import { registerSorafsOrderbookSubmissionTests } from "./sorafsNativeSuites/sorafsOrderbookSubmission.js";

registerSorafsOrderbookSubmissionTests({
  test,
  subject: { LocalSigningContext, SorafsOrderbookSubmissionAmbiguousError, ToriiClient, NetworkId, TORII_TEST_NATIVE_BINDING },
});
