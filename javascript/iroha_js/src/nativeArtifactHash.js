import { createHash } from "node:crypto";
import { Buffer } from "node:buffer";

const MACH_O_64_LE_MAGIC = 0xfeedfacf;
const MACH_O_64_HEADER_BYTES = 32;
const LC_SEGMENT_64 = 0x19;
const LC_CODE_SIGNATURE = 0x1d;
const MAX_LOAD_COMMANDS = 4096;
const PE_DOS_HEADER_BYTES = 64;
const PE_SIGNATURE_BYTES = Buffer.from([0x50, 0x45, 0x00, 0x00]);
const PE_COFF_HEADER_BYTES = 20;
const PE_OPTIONAL_MAGIC_32 = 0x10b;
const PE_OPTIONAL_MAGIC_64 = 0x20b;
const PE_CHECKSUM_OFFSET = 64;
const PE_CERTIFICATE_DIRECTORY_INDEX = 4;
const PE_CERTIFICATE_ALIGNMENT = 8;

function fail(message) {
  throw new TypeError(`invalid signed Mach-O native binding: ${message}`);
}

function readU32(bytes, offset, label) {
  if (offset < 0 || offset + 4 > bytes.length) fail(`${label} is out of bounds`);
  return bytes.readUInt32LE(offset);
}

function failPE(message) {
  throw new TypeError(`invalid Authenticode PE native binding: ${message}`);
}

function readPeU16(bytes, offset, limit, label) {
  if (offset < 0 || offset + 2 > limit || offset + 2 > bytes.length) {
    failPE(`${label} is out of bounds`);
  }
  return bytes.readUInt16LE(offset);
}

function readPeU32(bytes, offset, limit, label) {
  if (offset < 0 || offset + 4 > limit || offset + 4 > bytes.length) {
    failPE(`${label} is out of bounds`);
  }
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

/**
 * Return a signing-identity-independent SHA-256 for one PE/COFF image, or
 * `null` when `bytes` is not that file format.
 *
 * `unsignedSize` is the exact byte length recorded before Authenticode
 * signing. Signing may change only the PE checksum, the certificate-table
 * directory entry, up to seven zero alignment bytes after `unsignedSize`, and
 * one final non-empty WIN_CERTIFICATE table. A caller that accepts this digest
 * must still validate Authenticode with the operating system before loading
 * the image.
 */
export function peSigningIndependentSHA256(
  bytes,
  unsignedSize = bytes?.length,
  { requireSigned = false } = {},
) {
  if (!Buffer.isBuffer(bytes)) {
    throw new TypeError("native artifact bytes must be a Buffer");
  }
  if (bytes.length < 2 || bytes[0] !== 0x4d || bytes[1] !== 0x5a) {
    return null;
  }
  if (bytes.length < PE_DOS_HEADER_BYTES) failPE("DOS header is truncated");
  if (
    !Number.isSafeInteger(unsignedSize) ||
    unsignedSize < PE_DOS_HEADER_BYTES ||
    unsignedSize > bytes.length
  ) {
    failPE("unsigned byte length is outside the file bounds");
  }
  if (typeof requireSigned !== "boolean") {
    throw new TypeError("requireSigned must be a boolean");
  }

  const peOffset = readPeU32(bytes, 0x3c, unsignedSize, "PE header offset");
  if (
    peOffset < PE_DOS_HEADER_BYTES ||
    peOffset + PE_SIGNATURE_BYTES.length + PE_COFF_HEADER_BYTES > unsignedSize
  ) {
    failPE("PE/COFF header is out of bounds");
  }
  if (!bytes.subarray(peOffset, peOffset + 4).equals(PE_SIGNATURE_BYTES)) {
    failPE("PE signature is missing");
  }

  const coffOffset = peOffset + PE_SIGNATURE_BYTES.length;
  const optionalBytes = readPeU16(
    bytes,
    coffOffset + 16,
    unsignedSize,
    "optional-header byte length",
  );
  const optionalOffset = coffOffset + PE_COFF_HEADER_BYTES;
  const optionalEnd = optionalOffset + optionalBytes;
  if (optionalBytes === 0 || optionalEnd > unsignedSize) {
    failPE("optional header is out of bounds");
  }
  const magic = readPeU16(bytes, optionalOffset, optionalEnd, "optional-header magic");
  let directoryCountOffset;
  let directoryOffset;
  if (magic === PE_OPTIONAL_MAGIC_32) {
    directoryCountOffset = optionalOffset + 92;
    directoryOffset = optionalOffset + 96;
  } else if (magic === PE_OPTIONAL_MAGIC_64) {
    directoryCountOffset = optionalOffset + 108;
    directoryOffset = optionalOffset + 112;
  } else {
    failPE("optional-header magic is unsupported");
  }
  const directoryCount = readPeU32(
    bytes,
    directoryCountOffset,
    optionalEnd,
    "data-directory count",
  );
  if (directoryCount <= PE_CERTIFICATE_DIRECTORY_INDEX) {
    failPE("certificate-table directory is absent");
  }
  const certificateDirectory =
    directoryOffset + PE_CERTIFICATE_DIRECTORY_INDEX * 8;
  const certificateOffset = readPeU32(
    bytes,
    certificateDirectory,
    optionalEnd,
    "certificate-table file offset",
  );
  const certificateBytes = readPeU32(
    bytes,
    certificateDirectory + 4,
    optionalEnd,
    "certificate-table byte length",
  );
  const checksumOffset = optionalOffset + PE_CHECKSUM_OFFSET;
  readPeU32(bytes, checksumOffset, optionalEnd, "PE checksum");

  if (certificateOffset === 0 || certificateBytes === 0) {
    if (
      certificateOffset !== 0 ||
      certificateBytes !== 0 ||
      unsignedSize !== bytes.length
    ) {
      failPE("unsigned certificate-table layout is inconsistent");
    }
    if (requireSigned) failPE("Authenticode certificate table is absent");
  } else {
    if (
      certificateOffset % PE_CERTIFICATE_ALIGNMENT !== 0 ||
      certificateBytes < 8 ||
      certificateBytes % PE_CERTIFICATE_ALIGNMENT !== 0 ||
      certificateOffset < unsignedSize ||
      certificateOffset - unsignedSize >= PE_CERTIFICATE_ALIGNMENT ||
      certificateOffset + certificateBytes !== bytes.length
    ) {
      failPE("certificate table is not the final aligned file region");
    }
    if (
      !bytes
        .subarray(unsignedSize, certificateOffset)
        .every((byte) => byte === 0)
    ) {
      failPE("certificate alignment padding is non-zero");
    }
  }

  const normalized = Buffer.from(bytes.subarray(0, unsignedSize));
  normalized.fill(0, checksumOffset, checksumOffset + 4);
  normalized.fill(0, certificateDirectory, certificateDirectory + 8);
  return createHash("sha256").update(normalized).digest("hex");
}
