import { rejectType } from "./validationThrow.js";
import { isCanonicalKotodamaEntrypoint } from "./kotodamaIdentifiers.js";

/** Validate the current field set before reading values; preserve the caller's error boundary. */
export function validateManifestFieldsV1(manifest, context, reject = rejectType) {
  const fields = [
    ["seiyaku_name", "seiyakuName"], ["code_hash", "codeHash"],
    ["abi_hash", "abiHash"], ["compiler_fingerprint", "compilerFingerprint"],
    ["features_bitmap", "featuresBitmap"], ["access_set_hints", "accessSetHints"],
    ["entrypoints", "entryPoints"], ["error_types", "errorTypes"],
    ["error_messages", "errorMessages"], ["states"], ["kotoba"], ["provenance"],
  ];
  const allowed = new Set(fields.flat());
  const unknown = Object.keys(manifest).filter((key) => !allowed.has(key));
  if (unknown.length !== 0) {
    reject(`${context} contains unsupported fields: ${unknown.sort().join(", ")}`);
  }
  for (const aliases of fields) {
    const present = aliases.filter((key) => Object.hasOwn(manifest, key));
    if (present.length > 1) {
      reject(`${context} contains conflicting aliases: ${present.join(", ")}`);
    }
  }
}

/** Require the declared entrypoint kind and permission to match its canonical selector. */
export function validateManifestEntrypointIdentityV1(name, kind, permission, context) {
  if (!isCanonicalKotodamaEntrypoint(name)) {
    rejectType(`${context}.name must be a canonical Kotodama V1 identifier or branded lifecycle selector`);
  }
  const lifecycleKind =
    name === "hajimari" || name === "始まり"
      ? "Hajimari"
      : name === "kaizen" || name === "改善"
        ? "Kaizen"
        : null;
  if (
    (lifecycleKind !== null && kind !== lifecycleKind) ||
    (lifecycleKind === null && (kind === "Hajimari" || kind === "Kaizen"))
  ) {
    rejectType(`${context}.kind does not match its branded lifecycle selector`);
  }
  if (kind === "Kotoage" && permission === null) {
    rejectType(`${context}.permission is required for kotoage/言挙げ`);
  }
  if ((kind === "Hajimari" || kind === "Kaizen") && permission !== null) {
    rejectType(`${context}.permission must be null for hajimari/始まり and kaizen/改善`);
  }
}

/** Validate cross-declaration names, callbacks and access-hint completeness after normalization. */
export function validateManifestDeclarationsV1(manifest, context) {
  const entrypointKinds = new Map();
  const entrypointNames = new Set();
  const lifecycleKinds = new Set();
  const triggerIds = new Set();
  for (const [index, entrypoint] of (manifest.entrypoints ?? []).entries()) {
    if (entrypointNames.has(entrypoint.name)) {
      rejectType(`${context}.entrypoints contains duplicate name ${entrypoint.name}`);
    }
    entrypointNames.add(entrypoint.name);
    entrypointKinds.set(entrypoint.name, entrypoint.kind.kind);
    if (entrypoint.kind.kind === "Hajimari" || entrypoint.kind.kind === "Kaizen") {
      if (lifecycleKinds.has(entrypoint.kind.kind)) {
        rejectType(`${context}.entrypoints contains duplicate ${entrypoint.kind.kind} declarations`);
      }
      lifecycleKinds.add(entrypoint.kind.kind);
    }
    for (const trigger of entrypoint.triggers) {
      if (triggerIds.has(trigger.id)) {
        rejectType(`${context}.entrypoints contains duplicate trigger ${trigger.id}`);
      }
      triggerIds.add(trigger.id);
    }
    if (
      entrypoint.access_hints_complete === true &&
      entrypoint.access_hints_skipped.length !== 0
    ) {
      rejectType(`${context}.entrypoints[${index}] marks access hints complete but records skipped reasons`);
    }
    if (
      entrypoint.access_hints_complete === false &&
      entrypoint.access_hints_skipped.length === 0
    ) {
      rejectType(`${context}.entrypoints[${index}] marks access hints incomplete without a reason`);
    }
  }
  for (const [entrypointIndex, entrypoint] of (manifest.entrypoints ?? []).entries()) {
    for (const [triggerIndex, trigger] of entrypoint.triggers.entries()) {
      if (trigger.callback.namespace === null) {
        const targetKind = entrypointKinds.get(trigger.callback.entrypoint);
        if (targetKind === undefined) {
          rejectType(`${context}.entrypoints[${entrypointIndex}].triggers[${triggerIndex}] targets an undeclared local entrypoint`);
        }
        if (targetKind !== "Kotoage") {
          rejectType(`${context}.entrypoints[${entrypointIndex}].triggers[${triggerIndex}] local callback must target kotoage/言挙げ`);
        }
      }
    }
  }

  const stateNames = new Set();
  for (const state of manifest.states ?? []) {
    if (stateNames.has(state.name)) {
      rejectType(`${context}.states contains duplicate name ${state.name}`);
    }
    stateNames.add(state.name);
  }

  const messageIds = new Set();
  for (const [entryIndex, entry] of (manifest.kotoba ?? []).entries()) {
    if (messageIds.has(entry.msg_id)) {
      rejectType(`${context}.kotoba contains duplicate msg_id ${entry.msg_id}`);
    }
    messageIds.add(entry.msg_id);
    const languages = new Set();
    for (const translation of entry.translations) {
      if (languages.has(translation.lang)) {
        rejectType(`${context}.kotoba[${entryIndex}] contains duplicate language ${translation.lang}`);
      }
      languages.add(translation.lang);
    }
  }
}
