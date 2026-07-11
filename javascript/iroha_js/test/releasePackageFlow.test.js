import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const provenanceScript = readFileSync(
  new URL("../scripts/record-release-provenance.mjs", import.meta.url),
  "utf8",
);

test("publish lifecycle builds dist before tests and verifies the immutable packed install", () => {
  const lifecycle = packageJson.scripts.prepublishOnly;
  const expectedOrder = [
    "npm run check:changelog",
    "npm run build:dist",
    "npm run lint:test",
    "npm run bundle:check",
    "npm run test:pack-install",
    "npm run release:provenance",
  ];
  let previous = -1;
  for (const command of expectedOrder) {
    const index = lifecycle.indexOf(command);
    assert.ok(index > previous, `${command} must appear in release order after its predecessor`);
    previous = index;
  }
  assert.equal(packageJson.scripts["test:pack-install"], "node ./scripts/package-install-smoke.mjs");
  assert.equal(packageJson.scripts["release:publish"], "npm publish --access public --provenance");
  assert.equal(packageJson.scripts.prepare, undefined);
  assert.match(
    provenanceScript,
    /\["pack", "--ignore-scripts", "--json", "--pack-destination", packDir\]/u,
    "provenance packing inside prepublish must not recursively run prepublishOnly",
  );
});
