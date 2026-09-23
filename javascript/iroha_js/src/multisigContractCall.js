import { defaultNativeRuntime, resolveNativeRuntimeBinding } from "./nativeRuntime.js";
import { requireNetworkPrefix } from "./networkPrefix.js";

/** Build current exact native contract-multisig material from independently reviewed inputs.
 * This is a pure codec. It does not authenticate a contract deployment or finality.
 */
export function buildCanonicalMultisigContractCall(input, networkPrefix) {
  requireNetworkPrefix(networkPrefix, "networkPrefix");
  const native = resolveNativeRuntimeBinding(defaultNativeRuntime);
  if (typeof native?.buildCanonicalMultisigContractCallJson !== "function") {
    throw new Error("current native contract-multisig constructor is unavailable");
  }
  const encoded = JSON.stringify(input);
  if (typeof encoded !== "string" || encoded.length > 4 * 1024 * 1024) {
    throw new TypeError("bounded exact contract-multisig input required");
  }
  return JSON.parse(native.buildCanonicalMultisigContractCallJson(encoded, networkPrefix));
}
