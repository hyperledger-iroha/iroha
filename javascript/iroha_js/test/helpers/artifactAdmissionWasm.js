import { createHash } from "node:crypto";

import { instantiateIvmArtifactAdmissionWasm } from "../../src/ivmArtifactAdmissionWasm.js";

function unsignedLeb(value) {
  const output = [];
  let remaining = value >>> 0;
  do {
    let byte = remaining & 0x7f;
    remaining >>>= 7;
    if (remaining !== 0) byte |= 0x80;
    output.push(byte);
  } while (remaining !== 0);
  return output;
}

function signedLeb(value) {
  const output = [];
  let remaining = value | 0;
  let done = false;
  while (!done) {
    let byte = remaining & 0x7f;
    remaining >>= 7;
    done =
      (remaining === 0 && (byte & 0x40) === 0) ||
      (remaining === -1 && (byte & 0x40) !== 0);
    if (!done) byte |= 0x80;
    output.push(byte);
  }
  return output;
}

function vector(entries) {
  return [...unsignedLeb(entries.length), ...entries.flat()];
}

function name(value) {
  const bytes = Buffer.from(value, "utf8");
  return [...unsignedLeb(bytes.length), ...bytes];
}

function section(id, payload) {
  return [id, ...unsignedLeb(payload.length), ...payload];
}

function functionBody(instructions) {
  const body = [0, ...instructions, 0x0b];
  return [...unsignedLeb(body.length), ...body];
}

/** Build a tiny deterministic raw-ABI module for JavaScript boundary tests. */
export function staticArtifactAdmissionWasm(result) {
  const output = Buffer.from(JSON.stringify(result), "utf8");
  const outputPointer = 16_384;
  const typeSection = section(
    1,
    vector([
      [0x60, 0x00, 0x01, 0x7f],
      [0x60, 0x01, 0x7f, 0x01, 0x7f],
    ]),
  );
  const functionSection = section(3, vector([[0], [1], [0], [0]]));
  const memorySection = section(5, vector([[0x00, 0x01]]));
  const exportSection = section(
    7,
    vector([
      [...name("memory"), 0x02, 0x00],
      [
        ...name("iroha_ivm_artifact_admission_input_ptr"),
        0x00,
        0x00,
      ],
      [
        ...name("iroha_ivm_artifact_admission_verify"),
        0x00,
        0x01,
      ],
      [
        ...name("iroha_ivm_artifact_admission_output_ptr"),
        0x00,
        0x02,
      ],
      [
        ...name("iroha_ivm_artifact_admission_output_len"),
        0x00,
        0x03,
      ],
    ]),
  );
  const codeSection = section(
    10,
    vector([
      functionBody([0x41, ...signedLeb(0)]),
      functionBody([0x41, ...signedLeb(result.ok ? 1 : 0)]),
      functionBody([0x41, ...signedLeb(outputPointer)]),
      functionBody([0x41, ...signedLeb(output.length)]),
    ]),
  );
  const dataSection = section(
    11,
    vector([
      [
        0x00,
        0x41,
        ...signedLeb(outputPointer),
        0x0b,
        ...unsignedLeb(output.length),
        ...output,
      ],
    ]),
  );
  return Uint8Array.from([
    0x00,
    0x61,
    0x73,
    0x6d,
    0x01,
    0x00,
    0x00,
    0x00,
    ...typeSection,
    ...functionSection,
    ...memorySection,
    ...exportSection,
    ...codeSection,
    ...dataSection,
  ]);
}

export async function createStaticArtifactAdmissionVerifier(result) {
  const wasmBytes = staticArtifactAdmissionWasm(result);
  const expectedSha256Hex = createHash("sha256").update(wasmBytes).digest("hex");
  return instantiateIvmArtifactAdmissionWasm({ wasmBytes, expectedSha256Hex });
}
