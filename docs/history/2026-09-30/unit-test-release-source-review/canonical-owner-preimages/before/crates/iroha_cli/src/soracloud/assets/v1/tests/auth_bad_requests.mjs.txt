
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function assertRejects(promiseFactory, expectedMessage) {
  try {
    await promiseFactory();
  } catch (error) {
    if (String(error?.message ?? error).includes(expectedMessage)) {
      return;
    }
    throw new Error(`unexpected error: ${error?.stack ?? String(error)}`);
  }
  throw new Error(`expected rejection containing: ${expectedMessage}`);
}

function reqFromChunks(chunks, headers = { host: "clinic.test" }) {
  return {
    headers,
    [Symbol.asyncIterator]: async function* () {
      for (const chunk of chunks) {
        yield Buffer.from(chunk, "utf8");
      }
    }
  };
}

function resCapture() {
  return {
    status: null,
    headers: {},
    body: "",
    writeHead(status, headers = {}) {
      this.status = status;
      this.headers = headers;
    },
    end(body = "") {
      this.body += body ?? "";
    },
    json() {
      return this.body.length > 0 ? JSON.parse(this.body) : null;
    }
  };
}

await assertRejects(
  () => readJson(reqFromChunks(["{not-json"])),
  "invalid JSON payload"
);
await assertRejects(
  () => readJson(reqFromChunks(["x".repeat(65537)])),
  "request body too large"
);

const badChallengeJson = resCapture();
await handleAuthChallenge(reqFromChunks(["{not-json"]), badChallengeJson);
assert(badChallengeJson.status === 400, `invalid challenge JSON should fail: ${badChallengeJson.body}`);
assert(badChallengeJson.json().code === "INVALID_REQUEST", "invalid challenge JSON should return INVALID_REQUEST");

const missingPublicKey = resCapture();
await handleAuthChallenge(reqFromChunks([JSON.stringify({})]), missingPublicKey);
assert(missingPublicKey.status === 400, `missing public key should fail: ${missingPublicKey.body}`);
assert(missingPublicKey.json().error === "public_key must be a string", "missing public key error mismatch");

const badLoginJson = resCapture();
await handleAuthLogin(reqFromChunks(["{not-json"]), badLoginJson, new Map());
assert(badLoginJson.status === 400, `invalid login JSON should fail: ${badLoginJson.body}`);
assert(badLoginJson.json().code === "INVALID_REQUEST", "invalid login JSON should return INVALID_REQUEST");

const missingChallengeId = resCapture();
await handleAuthLogin(
  reqFromChunks([JSON.stringify({ public_key: "11".repeat(32), signature: "00".repeat(64) })]),
  missingChallengeId,
  new Map()
);
assert(missingChallengeId.status === 400, `missing challenge id should fail: ${missingChallengeId.body}`);
assert(missingChallengeId.json().error === "challenge_id must be a string", "missing challenge id error mismatch");
