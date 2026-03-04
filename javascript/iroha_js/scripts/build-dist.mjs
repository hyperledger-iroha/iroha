import { cp, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const ROOT = resolve(__dirname, "..");
const DIST = join(ROOT, "dist");
const SRC = join(ROOT, "src");

async function main() {
  await rm(DIST, { recursive: true, force: true });
  await cp(SRC, DIST, { recursive: true });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
