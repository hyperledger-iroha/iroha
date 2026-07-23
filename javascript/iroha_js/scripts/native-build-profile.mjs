export const NATIVE_BUILD_PROFILE_ENV = "IROHA_JS_NATIVE_BUILD_PROFILE";

const NATIVE_BUILD_PROFILES = new Set(["debug", "release", "deploy"]);

/** Resolve the Cargo profile used to build and publish the native addon. */
export function resolveNativeBuildProfile(environment = process.env) {
  const configured = environment[NATIVE_BUILD_PROFILE_ENV];
  const profile = configured === undefined ? "debug" : configured;
  if (!NATIVE_BUILD_PROFILES.has(profile)) {
    throw new TypeError(
      `${NATIVE_BUILD_PROFILE_ENV} must be exactly "debug", "release", or "deploy"`,
    );
  }
  return profile;
}

export function cargoBuildArgsForNativeProfile(profile) {
  if (!NATIVE_BUILD_PROFILES.has(profile)) {
    throw new TypeError(
      'native build profile must be exactly "debug", "release", or "deploy"',
    );
  }
  if (profile === "deploy") return ["--profile", "deploy"];
  return profile === "release" ? ["--release"] : [];
}
