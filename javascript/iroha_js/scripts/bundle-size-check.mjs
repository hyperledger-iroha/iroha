import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const ROOT = resolve(__dirname, "..");

async function main() {
  let esbuild;
  try {
    esbuild = await import("esbuild");
  } catch (error) {
    console.warn(
      "bundle-size-check: esbuild not installed, skipping bundle+tree-shake check (install esbuild to enforce the limit).",
    );
  }

  if (esbuild?.build) {
    const result = await esbuild.build({
      entryPoints: [join(ROOT, "src", "toriiClient.js")],
      bundle: true,
      write: false,
      platform: "node",
      target: "node18",
      format: "esm",
      treeShaking: true,
      sourcemap: false,
      minify: true,
    });

    const bundle = result.outputFiles[0]?.text ?? "";
    const bytes = Buffer.byteLength(bundle, "utf8");
    const kb = (bytes / 1024).toFixed(1);
    const limitKb = 300;
    console.log(`Bundled toriiClient.js: ${kb} KiB (limit ${limitKb} KiB)`);
    if (bytes > limitKb * 1024) {
      throw new Error(`bundle size ${kb} KiB exceeds limit ${limitKb} KiB`);
    }
  }

  const pkgPath = join(ROOT, "package.json");
  const pkg = JSON.parse(await readFile(pkgPath, "utf8"));
  const toriiExport = pkg.exports?.["./torii"] ?? pkg.exports?.torii;
  const toriiTarget =
    typeof toriiExport === "string" ? toriiExport : toriiExport?.import;
  if (!toriiTarget?.startsWith("./dist/")) {
    throw new Error("torii subpath export should point to built dist artifacts");
  }

  const toriiDist = resolve(ROOT, toriiTarget);
  try {
    await readFile(toriiDist, "utf8");
  } catch (error) {
    throw new Error(
      `torii subpath export points to ${pathToFileURL(toriiDist)}, but the file is missing. Run npm run build:dist.`,
    );
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
