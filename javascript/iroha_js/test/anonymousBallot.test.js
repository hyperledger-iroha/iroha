import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { once } from "node:events";
import { readFileSync } from "node:fs";
import { AnonymousBallotClientV1, AnonymousBallotRefusal } from "../src/anonymousBallot.js";

test("anonymous ballot transport preserves whole records and carries no caller credentials", async (t) => {
  const requests = [];
  const fixture = (name) => readFileSync(new URL("../../../fixtures/governance/anonymous_ballot_http_v1/" + name, import.meta.url));
  const ballotFrame = fixture("ballot.nrt"), boardFrame = fixture("board.nrt"), attemptFrame = fixture("attempt.nrt");
  const server = http.createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requests.push({ method: request.method, url: request.url, headers: request.headers, body: Buffer.concat(chunks) });
    const mode = request.url.split("/")[1];
    if (mode === "compressed" || mode === "oversized") {
      const shape = mode === "compressed" ? { "Content-Encoding": "gzip" } : { "Content-Length": "65537" };
      response.writeHead(503, { ...shape, "Retry-After": "7" }).end();
      return;
    }
    if (mode === "redirect") { response.writeHead(307, { Location: "/unexpected" }).end(); return; }
    if (mode === "refuse") {
      response.writeHead(429, { "Content-Type": "application/json", "Retry-After": "3" })
        .end('{"code":"anonymous_ballot_throttled","message":"untrusted text"}');
      return;
    }
    if (request.method === "POST") {
      response.writeHead(mode === "wrong" ? 200 : 202, { "Set-Cookie": "identity=never" }).end(mode === "body" ? "x" : "");
      return;
    }
    if (mode === "large") { response.writeHead(200, { "Content-Type": "application/x-norito", "Content-Length": "1048577" }).end(); return; }
    response.writeHead(200, { "Content-Type": mode === "wrong" ? "application/json" : "application/x-norito" })
      .end(mode === "empty" ? "" : (request.url.includes("/boards/") ? boardFrame : attemptFrame));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => { server.closeAllConnections(); server.close(); });
  const endpoint = `http://127.0.0.1:${server.address().port}`;
  const client = (prefix) => new AnonymousBallotClientV1(endpoint + "/" + prefix, { allowLocalHttp: true });
  assert.throws(() => new AnonymousBallotClientV1(endpoint));
  assert.throws(() => new AnonymousBallotClientV1("https://account:secret@example.invalid"));
  assert.throws(() => new AnonymousBallotClientV1("https://example.invalid/?token=value"));
  const scope = "856bb8bcfa0804b4df16c68d0aa758937016ac87a3e783fa4b434ebeb5f4cd584e4abe3bf0b859b89863fb653f620aba7b7ea2ac4343c8723a7021a15cc01d2d";
  const board = Buffer.from(scope, "hex"), attempt = "c53a4fa9eae4964c1f6b2e60740ee721971fa82748df98af93bc606bea20fcc4";
  const api = client("good");
  await api.submitAnonymousBallot(ballotFrame);
  assert.deepEqual(await api.getAnonymousBallotBoard(board), new Uint8Array(boardFrame));
  assert.deepEqual(await api.getAnonymousParliamentAttempt(attempt), new Uint8Array(attemptFrame));
  assert.deepEqual(requests.map(({ method, url }) => [method, url]), [
    ["POST", "/good/v1/gov/anonymous-ballots"], ["GET", "/good/v1/gov/anonymous-ballots/boards/" + scope],
    ["GET", "/good/v1/gov/anonymous-ballots/attempts/" + attempt],
  ]);
  assert.deepEqual(requests[0].body, ballotFrame);
  assert.equal(requests[0].headers["content-type"], "application/x-norito");
  for (const { headers } of requests) {
    for (const name of ["authorization", "cookie", "x-api-token", "x-iroha-account", "referer"]) assert.equal(headers[name], undefined);
  }
  await assert.rejects(api.submitAnonymousBallot(new Uint8Array(262145)));
  assert.throws(() => api.getAnonymousBallotBoard("AB".repeat(64)));
  for (const mode of ["redirect", "wrong", "body"]) await assert.rejects(client(mode).submitAnonymousBallot(new Uint8Array([1])));
  for (const mode of ["wrong", "empty", "large"]) await assert.rejects(client(mode).getAnonymousBallotBoard(board));
  await assert.rejects(client("refuse").submitAnonymousBallot(new Uint8Array([1])), (e) =>
    e instanceof AnonymousBallotRefusal && e.status === 429 && e.code === "anonymous_ballot_throttled" && e.retryAfterSeconds === 3 && !e.message.includes("untrusted"));
  for (const mode of ["compressed", "oversized"]) {
    await assert.rejects(client(mode).submitAnonymousBallot(ballotFrame), (error) =>
      error instanceof AnonymousBallotRefusal && error.status === 503 && error.code === null && error.retryAfterSeconds === 7);
  }
  assert.ok(requests.every(({ url }) => url !== "/unexpected"));
});
