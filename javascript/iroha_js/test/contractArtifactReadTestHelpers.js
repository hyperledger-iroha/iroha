import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext } from "../src/toriiClient.js";
import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";

/** Explicit immutable deployment identity shared by artifact-read transport fixtures. */
export const artifactReadNetwork = NetworkId.parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
/** Account request context, with runtime-only disposable test signing custody. */
export const artifactReadContext = new LocalSigningContext(artifactReadNetwork, 753);
/** Disposable account authentication for exact artifact-read requests. */
export function artifactReadOptions() {
  return { canonicalAuth: { accountId: "alice-1@wonderland", privateKey: Buffer.alloc(32, 0x0c) } };
}
/** Add the independently requested network and artifact identity to transport fixtures. */
export function withArtifactResponseIdentity(payload, codeHash) {
  const hash = typeof codeHash === "string" && codeHash.startsWith("hash:")
    ? codeHash : canonicalHashLiteral(Buffer.from(codeHash, "hex"));
  return { network_id: artifactReadNetwork.literal, artifact_id: { dataspace_id: "0", code_hash: hash }, ...payload };
}
