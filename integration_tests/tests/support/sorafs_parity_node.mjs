// Actual authenticated Node-binding parity runner; no implementation projections.
import { readFileSync } from "node:fs";
import { getNativeBinding } from "../../../javascript/iroha_js/src/native.js";

const request = JSON.parse(readFileSync(process.argv[2], "utf8"));
function camel(value) {
  if (Array.isArray(value)) return value.map(camel);
  if (value && typeof value === "object") return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [
      key.replace(/_([a-z])/gu, (_, letter) => letter.toUpperCase()), camel(entry),
    ]),
  );
  return value;
}
function snake(value) {
  if (Array.isArray(value)) return value.map(snake);
  if (value && typeof value === "object") return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [
      key.replace(/[A-Z]/gu, (letter) => `_${letter.toLowerCase()}`), snake(entry),
    ]),
  );
  return value;
}
const native = getNativeBinding(); // Checks artifact, ABI, source provenance and loaded-byte custody.
const providers = camel(request.providers);
const options = camel(request.options);
const start = process.hrtime.bigint();
const result = native.sorafsMultiFetchLocal(request.plan, providers, options);
const duration = process.hrtime.bigint() - start;
const { payload, ...report } = result;
process.stdout.write(JSON.stringify({
  payload_hex: Buffer.from(payload).toString("hex"),
  report: snake(report), duration_ns: Number(duration),
}));
