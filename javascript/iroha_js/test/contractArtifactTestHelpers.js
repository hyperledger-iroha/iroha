import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";

/** Explicit universal-dataspace fixture custody for existing content-validation cases. */
export function universalArtifactInput(input) {
  const manifest = input.manifest;
  const hash = input.codeHash ?? input.code_hash ?? manifest?.codeHash ?? manifest?.code_hash ?? "11".repeat(32);
  const result = { ...input, artifactId: { dataspaceId: "0", codeHash: hash } };
  delete result.codeHash;
  delete result.code_hash;
  if (manifest && typeof manifest === "object" && manifest.codeHash == null && manifest.code_hash == null) {
    const hashField = Object.hasOwn(manifest, "code_hash") ? "code_hash" : "codeHash";
    result.manifest = { ...manifest, [hashField]: hash };
  }
  return result;
}

/** Bind a canonical instruction fixture or expected value to the explicit universal scope. */
export function universalArtifactInstruction(instruction) {
  const [variant] = Object.keys(instruction);
  const payload = instruction[variant];
  const codeHash = payload.code_hash ?? payload.manifest?.code_hash ?? canonicalHashLiteral(Buffer.alloc(32, 0x11));
  return { [variant]: {
    ...Object.fromEntries(Object.entries(payload).filter(([key]) => key !== "code_hash")),
    artifact_id: { dataspace_id: "0", code_hash: codeHash },
  } };
}
