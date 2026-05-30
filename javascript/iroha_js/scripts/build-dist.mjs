import { cpSync, existsSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const ROOT = resolve(__dirname, "..");
const DIST = join(ROOT, "dist");
const SRC = join(ROOT, "src");

async function main() {
  rmSync(DIST, { recursive: true, force: true });
  cpSync(SRC, DIST, { recursive: true });
  const requiredOutputs = ["address.js", "curveRegistry.js", "toriiClient.js", "kotodamaCompiler/index.js"];
  for (const fileName of requiredOutputs) {
    if (!existsSync(join(DIST, fileName))) {
      throw new Error(`build:dist missing expected output: ${fileName}`);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
