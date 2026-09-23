import { test } from "node:test";
import os from "node:os";
import { getNativeBinding } from "../src/native.js";
import { nativeBinding, nativeBindingError } from "./helpers/native.js";
import { registerSorafsOrchestratorParityTests } from "./sorafsNativeSuites/sorafsOrchestrator.parity.js";

registerSorafsOrchestratorParityTests({
  test,
  subject: { getNativeBinding },
  fixtureRoot: new URL("../../../fixtures/sorafs_orchestrator/multi_peer_parity_v1/", import.meta.url),
  repositoryRoot: new URL("../../../", import.meta.url),
  temporaryRoot: os.tmpdir(),
  nativeBinding,
  nativeBindingError,
});
