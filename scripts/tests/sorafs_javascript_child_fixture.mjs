// Shared synthetic input fixture. Actual code/census originals are read as bytes;
// inert installed/tool rows are not an authenticated package or execution claim.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { CHILD_TOOL_FILES } from "../sorafs_javascript_child_input.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const CAT = readFileSync(join(ROOT, "scripts/fixtures/sorafs_javascript_qualification_sources_v1.json"));
const catalog = JSON.parse(CAT);
export const hash = (raw) => createHash("sha256").update(raw).digest("hex");
export function canonical(value) {
  const sorted = (item) => Array.isArray(item) ? item.map(sorted) : item !== null && typeof item === "object"
    ? Object.fromEntries(Object.keys(item).sort().map((key) => [key, sorted(item[key])])) : item;
  return Buffer.from(JSON.stringify(sorted(value)).replace(/[\u007f-\uffff]/g,
    (char) => "\\u" + char.charCodeAt(0).toString(16).padStart(4, "0")) + "\n");
}
export function fixture() {
  const source = [...catalog.source_files.map((row) => row.path), ...catalog.fixture_files].sort().map((path) => {
    const raw = readFileSync(join(ROOT, path)); return { path, sha256: hash(raw), size: raw.length, mode: 0o644 };
  });
  const inert = (path) => ({ path, sha256: hash(Buffer.from("inert " + path)), size: Buffer.byteLength("inert " + path), mode: 0o644 });
  return { schema: "sorafs.javascript.child_input.v1", environmentRoot: "/owned/environment", temporaryRoot: "/owned/temporary",
    native: { originalPath: "/owned/native/iroha_js_host.node", sha256: "a".repeat(64), size: 1,
      checksumSha256: "b".repeat(64), checksumSize: 1, sourceCommit: "c".repeat(40),
      nativeSourceTreeSha256: "d".repeat(64), workspaceSourceTreeSha256: "e".repeat(64) },
    catalog: CAT.toString("base64"), source,
    installed: [".package-lock.json", ...["dist/index.js", "dist/public/norito.js", "dist/public/sorafs.js", "dist/toriiClient.js",
      "dist/native.js", "dist/toriiTestHooks.js", "package.json"].map((path) => "@iroha/iroha-js/" + path)].sort().map(inert),
    tools: CHILD_TOOL_FILES.map(inert) };
}
