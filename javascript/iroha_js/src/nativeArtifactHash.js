import { createHash } from "node:crypto";
import { Buffer } from "node:buffer";

const MACH_O_64_LE_MAGIC = 0xfeedfacf;
const MACH_O_64_HEADER_BYTES = 32;
const LC_SEGMENT_64 = 0x19;
const LC_CODE_SIGNATURE = 0x1d;
const MAX_LOAD_COMMANDS = 4096;

function fail(message) {
  throw new TypeError(`invalid signed Mach-O native binding: ${message}`);
}

function readU32(bytes, offset, label) {
  if (offset < 0 || offset + 4 > bytes.length) fail(`${label} is out of bounds`);
  return bytes.readUInt32LE(offset);
}

function hasExactSegmentName(bytes, offset, name) {
  const field = bytes.subarray(offset, offset + 16);
  const expected = Buffer.from(name, "ascii");
  return (
    field.subarray(0, expected.length).equals(expected) &&
    field.subarray(expected.length).every((byte) => byte === 0)
  );
}

/**
 * Return a signing-identity-independent SHA-256 for one thin, little-endian
 * 64-bit Mach-O image, or `null` when `bytes` is not that file format.
 *
 * macOS re-signing changes only the embedded signature blob and the size of
 * the `__LINKEDIT`/`LC_CODE_SIGNATURE` containers. The digest retains every
 * other header, load command, and byte before the signature. A caller that
 * accepts this digest must still rely on macOS code-signature enforcement (or
 * run `codesign --verify`) before loading the image.
 */
export function machOSigningIndependentSHA256(bytes) {
  if (!Buffer.isBuffer(bytes)) {
    throw new TypeError("native artifact bytes must be a Buffer");
  }
  if (bytes.length < 4 || bytes.readUInt32LE(0) !== MACH_O_64_LE_MAGIC) {
    return null;
  }
  if (bytes.length < MACH_O_64_HEADER_BYTES) fail("header is truncated");

  const commandCount = readU32(bytes, 16, "load-command count");
  const commandBytes = readU32(bytes, 20, "load-command byte length");
  if (commandCount === 0 || commandCount > MAX_LOAD_COMMANDS) {
    fail("load-command count is outside the supported bound");
  }
  const commandEnd = MACH_O_64_HEADER_BYTES + commandBytes;
  if (commandEnd < MACH_O_64_HEADER_BYTES || commandEnd > bytes.length) {
    fail("load-command table is out of bounds");
  }

  let offset = MACH_O_64_HEADER_BYTES;
  let codeSignatureCommand;
  let linkeditSegment;
  for (let index = 0; index < commandCount; index += 1) {
    if (offset + 8 > commandEnd) fail("load-command header is truncated");
    const command = readU32(bytes, offset, "load-command type");
    const size = readU32(bytes, offset + 4, "load-command size");
    if (size < 8 || size % 8 !== 0 || offset + size > commandEnd) {
      fail("load-command size is non-canonical");
    }
    if (command === LC_CODE_SIGNATURE) {
      if (codeSignatureCommand !== undefined || size !== 16) {
        fail("code-signature command is missing or duplicated");
      }
      codeSignatureCommand = offset;
    } else if (
      command === LC_SEGMENT_64 &&
      size >= 72 &&
      hasExactSegmentName(bytes, offset + 8, "__LINKEDIT")
    ) {
      if (linkeditSegment !== undefined) fail("__LINKEDIT segment is duplicated");
      linkeditSegment = offset;
    }
    offset += size;
  }
  if (offset !== commandEnd) fail("load-command byte length is inconsistent");
  if (codeSignatureCommand === undefined || linkeditSegment === undefined) {
    fail("required code-signature layout is absent");
  }

  const signatureOffset = readU32(
    bytes,
    codeSignatureCommand + 8,
    "code-signature offset",
  );
  const signatureBytes = readU32(
    bytes,
    codeSignatureCommand + 12,
    "code-signature byte length",
  );
  if (
    signatureOffset < commandEnd ||
    signatureBytes === 0 ||
    signatureOffset + signatureBytes !== bytes.length
  ) {
    fail("code signature must be the final non-empty file region");
  }

  const linkeditFileOffset = Number(bytes.readBigUInt64LE(linkeditSegment + 40));
  const linkeditFileBytes = Number(bytes.readBigUInt64LE(linkeditSegment + 48));
  if (
    !Number.isSafeInteger(linkeditFileOffset) ||
    !Number.isSafeInteger(linkeditFileBytes) ||
    linkeditFileOffset > signatureOffset ||
    linkeditFileOffset + linkeditFileBytes !== bytes.length
  ) {
    fail("__LINKEDIT does not contain the final code signature");
  }

  const normalized = Buffer.from(bytes.subarray(0, signatureOffset));
  // A distribution signature may cross a page boundary, changing both the
  // virtual and file sizes of __LINKEDIT. Those sizes and the signature blob
  // length are the only mutable signing containers accepted by this profile.
  normalized.fill(0, linkeditSegment + 32, linkeditSegment + 40);
  normalized.fill(0, linkeditSegment + 48, linkeditSegment + 56);
  normalized.fill(0, codeSignatureCommand + 12, codeSignatureCommand + 16);
  return createHash("sha256").update(normalized).digest("hex");
}
