import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const clientSource = readFileSync(
  new URL("../src/toriiClient.js", import.meta.url),
  "utf8",
);
const nativeSource = readFileSync(
  new URL("../../../crates/iroha_js_host/src/privacy_exact12_action.rs", import.meta.url),
  "utf8",
);
const detailsSource = readFileSync(
  new URL(
    "../../../crates/iroha_js_host/src/authenticated_transaction_details.rs",
    import.meta.url,
  ),
  "utf8",
);
const receiptSource = readFileSync(
  new URL(
    "../../../crates/iroha_js_host/src/authenticated_privacy_action_receipt.rs",
    import.meta.url,
  ),
  "utf8",
);

function methodBody(name, nextName) {
  const start = clientSource.indexOf(`  async ${name}(`);
  const end = clientSource.indexOf(`  async ${nextName}(`, start + 1);
  assert.ok(start >= 0, `${name} must exist`);
  assert.ok(end > start, `${name} must end before ${nextName}`);
  return clientSource.slice(start, end);
}

test("Exact12 submit authenticates native wire before fresh manifest and canonical submission", () => {
  const body = methodBody(
    "submitSignedPrivacyActionV1",
    "getPrivacyActionStatusV1",
  );
  const inspection = body.indexOf("inspectSignedPrivacyActionNativeV1(");
  const manifest = body.indexOf("getPrivacyExact12CapabilityManifestV1(");
  const admission = body.indexOf("requirePrivacyExact12CapabilityTupleV1(");
  const submission = body.indexOf("_submitSignedPrivacyActionWireV1(");
  assert.ok(inspection >= 0);
  assert.ok(inspection < manifest);
  assert.ok(manifest < admission);
  assert.ok(admission < submission);
  assert.match(body, /expectedManifestDigest/u);
  assert.match(body, /timingSafeEqual/u);
  assert.match(
    body,
    /request\.operation,\s*normalized\.canonicalAuth\.accountId,\s*\)/u,
  );
});

test("Exact12 transport is one-shot exact-network canonical Torii", () => {
  const submit = methodBody(
    "_submitSignedPrivacyActionWireV1",
    "_getAuthenticatedPrivacyActionStatusV1",
  );
  assert.match(submit, /canonicalAuth: options\.canonicalAuth/u);
  assert.match(submit, /exactNetworkId: options\.networkId/u);
  assert.match(submit, /disableRetries: true/u);
  assert.match(submit, /redirect: "error"/u);
  assert.match(submit, /body: Buffer\.from\(wire\)/u);
});

test("Exact12 status accepts only client-bound native-inspected views", () => {
  const submit = methodBody(
    "submitSignedPrivacyActionV1",
    "getPrivacyActionStatusV1",
  );
  const status = methodBody(
    "getPrivacyActionStatusV1",
    "_submitSignedPrivacyActionWireV1",
  );
  const submitted = submit.indexOf("_submitSignedPrivacyActionWireV1(");
  const provenance = submit.indexOf("bindPrivacyActionViewProvenanceV1(");
  assert.ok(submitted >= 0);
  assert.ok(provenance > submitted);
  assert.ok(
    status.indexOf("requirePrivacyActionViewProvenanceV1(")
      < status.indexOf("_getAuthenticatedPrivacyActionStatusV1("),
  );
  assert.match(clientSource, /provenance\.client !== client/u);
  assert.match(clientSource, /inheritPrivacyActionViewProvenanceV1\(terminal, operation\)/u);
});

test("terminal status requires committed result and finalized native receipt", () => {
  const details = methodBody(
    "_getAuthenticatedPrivacyActionDetailsV1",
    "_getAuthenticatedPrivacyActionReceiptV1",
  );
  const receipt = methodBody(
    "_getAuthenticatedPrivacyActionReceiptV1",
    "_resolvePrivacyActionStatusV1",
  );
  const resolve = methodBody(
    "_resolvePrivacyActionStatusV1",
    "_waitForPrivacyActionTerminalV1",
  );
  assert.match(details, /privacyBuildFindCommittedTransactionQueryV1/u);
  assert.match(details, /Accept: APPLICATION_NORITO/u);
  assert.match(details, /response\.status === 404/u);
  assert.match(details, /return null/u);
  assert.match(details, /privacyInspectPipelineTransactionDetailsV1/u);
  assert.match(receipt, /privacyBuildFindPrivacyActionExecutionReceiptQueryV1/u);
  assert.match(receipt, /"\/v1\/query"/u);
  assert.match(receipt, /canonicalAuth: options\.canonicalAuth/u);
  assert.match(receipt, /exactNetworkId: options\.networkId/u);
  assert.match(receipt, /disableRetries: true/u);
  assert.match(receipt, /redirect: "error"/u);
  assert.match(receipt, /privacyInspectPrivacyActionExecutionReceiptResponseV1/u);
  assert.match(receipt, /operation\.transactionIntentDigest/u);
  assert.match(receipt, /operation\.statementDigest/u);
  assert.match(receipt, /operation\.proofEnvelopeHash/u);
  assert.match(resolve, /details\.resultOk/u);
  assert.match(resolve, /details\.rejectionMessage/u);
  assert.match(resolve, /receipt === null/u);
  assert.match(resolve, /receipt\.admittedAtHeight !== details\.committedHeight/u);
  assert.match(resolve, /kind === "Rejected" \? "Rejected" : "Applied"/u);
  assert.match(resolve, /status height differs from authenticated committed details/u);
  assert.match(resolve, /status height differs from finalized execution receipt/u);
  assert.match(resolve, /details === null \|\| receipt === null/u);
  assert.match(resolve, /resolution\.resolvedFrom === "cache"/u);
  assert.match(
    resolve,
    /kind === "Queued" \|\| kind === "Approved" \|\| kind === "Committed"/u,
  );
  assert.doesNotMatch(resolve, /new Set\(\["Committed", "Applied", "Rejected"\]\)/u);
  assert.match(resolve, /return operation/u);
  assert.match(clientSource, /executionCapabilityManifestDigest/u);
  assert.match(clientSource, /executionReceiptFinalizedBlockHash/u);
  assert.doesNotMatch(
    clientSource,
    /capabilityCommittedHeight !== operation\.capabilityCommittedHeight/u,
  );
});

test("native inspectors authenticate shape without local proof acceptance", () => {
  assert.match(nativeSource, /verify_signature\(\)/u);
  assert.match(nativeSource, /canonical_authority\(authority_literal\)/u);
  assert.match(nativeSource, /signed\.authority\(\) != &expected_authority/u);
  assert.match(nativeSource, /decode_all_versioned/u);
  assert.match(nativeSource, /privacy_transaction_intent_binding_if_present_v1/u);
  assert.match(nativeSource, /validate_zk_x509_credential_proof_container_v1/u);
  assert.doesNotMatch(nativeSource, /verify_privacy_proof/u);
  assert.match(detailsSource, /with_authority/u);
  assert.match(detailsSource, /try_sign/u);
  assert.match(detailsSource, /decode_canonical_with_limits/u);
  assert.match(detailsSource, /result_hash\(\)/u);
  assert.match(receiptSource, /FindPrivacyActionExecutionReceiptV1/u);
  assert.match(receiptSource, /norito::decode_canonical_with_limits/u);
  assert.match(receiptSource, /norito::canonical_decode_limits\(response\.len\(\)\)/u);
  assert.match(receiptSource, /norito::to_bytes\(&decoded\)/u);
  assert.match(receiptSource, /canonical != response/u);
  assert.match(receiptSource, /receipt\s*\.validate\(\)/u);
  assert.match(receiptSource, /PrivacyActionExecutionReceiptViewV1/u);
  assert.match(receiptSource, /receipt\.network_id != expected_network_id/u);
  assert.match(receiptSource, /receipt\.protocol_id != expected_protocol/u);
  assert.match(receiptSource, /receipt\.operation_schema != expected_operation/u);
  assert.match(receiptSource, /receipt\.ledger_effect_kind != expected_effect/u);
  assert.match(receiptSource, /receipt\.transaction_hash != \*expected_transaction_hash/u);
  assert.match(receiptSource, /receipt\.action_index != action_index/u);
  assert.match(receiptSource, /receipt\.transaction_intent_digest\.as_bytes\(\)/u);
  assert.match(receiptSource, /receipt\.statement_digest\.as_bytes\(\)/u);
  assert.match(
    receiptSource,
    /receipt\.proof_envelope_hash != expected_binding\.proof_envelope_hash/u,
  );
  assert.doesNotMatch(receiptSource, /verify_privacy_proof/u);
});
