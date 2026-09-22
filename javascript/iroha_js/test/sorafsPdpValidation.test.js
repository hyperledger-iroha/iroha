import { test } from "node:test";
import { SORAFS_PDP_PAYLOAD_KINDS, validatePdpBundle, validatePdpChallengeProof, validatePdpCommitmentChallenge, validatePdpPayload } from "../src/sorafs.js";
import { registerSorafsPdpValidationTests } from "./sorafsNativeSuites/sorafsPdpValidation.js";

registerSorafsPdpValidationTests({
  test,
  subject: { SORAFS_PDP_PAYLOAD_KINDS, validatePdpBundle, validatePdpChallengeProof, validatePdpCommitmentChallenge, validatePdpPayload },
  fixtureRoot: new URL("../../../fixtures/sorafs_manifest/", import.meta.url),
});
