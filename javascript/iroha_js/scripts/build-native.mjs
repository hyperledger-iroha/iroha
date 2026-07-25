#!/usr/bin/env node
/** Build the native `iroha_js_host` library and bind it to Git provenance. */
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  cargoBuildArgsForNativeProfile,
  resolveNativeBuildProfile,
} from "./native-build-profile.mjs";
import {
  createNativeBuildProvenance,
  readNativeBuildSourceState,
  writeNativeBuildProvenance,
} from "./native-build-provenance.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = join(scriptDir, "..", "..", "..");

export function nativeBuildOutputPath({
  repoRoot = defaultRepoRoot,
  cargoProfile,
  env = process.env,
  platform = process.platform,
}) {
  const configuredTarget = env.CARGO_TARGET_DIR;
  const targetRoot = configuredTarget
    ? isAbsolute(configuredTarget)
      ? configuredTarget
      : resolve(repoRoot, configuredTarget)
    : join(repoRoot, "target");
  const filename =
    platform === "win32"
      ? "iroha_js_host.dll"
      : `libiroha_js_host.${platform === "darwin" ? "dylib" : "so"}`;
  return join(targetRoot, cargoProfile, filename);
}

export function runNativeBuild({
  repoRoot = defaultRepoRoot,
  env = process.env,
  platform = process.platform,
  runCargo = (args) =>
    spawnSync("cargo", args, {
      cwd: repoRoot,
      stdio: "inherit",
      env,
    }),
  readSourceState = readNativeBuildSourceState,
  writeProvenance = writeNativeBuildProvenance,
} = {}) {
  const cargoProfile = resolveNativeBuildProfile(env);
  const cargoManifest = join(repoRoot, "Cargo.toml");
  const buildArgs = [
    "build",
    "--locked",
    "--manifest-path",
    cargoManifest,
    "-p",
    "iroha_js_host",
    ...cargoBuildArgsForNativeProfile(cargoProfile),
  ];
  const sourceBefore = readSourceState(repoRoot);
  const build = runCargo(buildArgs);
  if (build.status !== 0) return build.status ?? 1;
  const sourceAfter = readSourceState(repoRoot);
  const nativePath = nativeBuildOutputPath({
    repoRoot,
    cargoProfile,
    env,
    platform,
  });
  const provenance = createNativeBuildProvenance({
    cargoProfile,
    nativePath,
    sourceBefore,
    sourceAfter,
  });
  writeProvenance(nativePath, provenance);
  return 0;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.exitCode = runNativeBuild();
}
