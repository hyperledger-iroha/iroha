// SPDX-License-Identifier: Apache-2.0

import {constants as fsConstants} from 'node:fs';
import {randomBytes} from 'node:crypto';
import {lstat, open, rename, unlink} from 'node:fs/promises';
import path from 'node:path';
import {TextDecoder} from 'node:util';

const READ_CHUNK_BYTES = 64 * 1024;

/**
 * Read one bounded OpenAPI control-plane input from a pinned descriptor.
 *
 * The reader rejects symlink or multi-link targets, detects in-place mutation,
 * revalidates the pathname and its ancestors after the read, and can enforce
 * non-writable trust-root permissions on Unix.
 */
export async function readOpenApiStableFile(
  filePath,
  {
    label = 'OpenAPI input',
    maxBytes,
    encoding = null,
    requireSafePermissions = false,
  } = {},
) {
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 0
  ) {
    throw new TypeError(`${label} maxBytes must be a non-negative safe integer`);
  }
  const resolved = path.resolve(filePath);
  await validateAncestors(resolved, label);
  const pathBefore = await inspectPath(resolved, label);

  const flags =
    fsConstants.O_RDONLY |
    (typeof fsConstants.O_NOFOLLOW === 'number' ? fsConstants.O_NOFOLLOW : 0);
  let handle;
  try {
    handle = await open(resolved, flags);
  } catch (error) {
    const wrapped = new Error(
      `failed to open ${label} ${resolved} without following symlinks: ${error.message ?? error}`,
      {cause: error},
    );
    wrapped.code = error?.code;
    throw wrapped;
  }

  try {
    const openedBefore = await handle.stat({bigint: true});
    validateOpenedFile(openedBefore, pathBefore, resolved, label);
    if (requireSafePermissions) {
      validateSafePermissions(openedBefore, resolved, label);
    }
    if (openedBefore.size > BigInt(maxBytes)) {
      throw new Error(`${label} ${resolved} exceeds the ${maxBytes}-byte limit`);
    }

    const chunks = [];
    let offset = 0;
    while (offset <= maxBytes) {
      const remaining = maxBytes + 1 - offset;
      if (remaining === 0) {
        break;
      }
      const buffer = Buffer.allocUnsafe(Math.min(READ_CHUNK_BYTES, remaining));
      const {bytesRead} = await handle.read(buffer, 0, buffer.length, offset);
      if (bytesRead === 0) {
        break;
      }
      chunks.push(buffer.subarray(0, bytesRead));
      offset += bytesRead;
    }
    if (offset > maxBytes) {
      throw new Error(`${label} ${resolved} exceeds the ${maxBytes}-byte limit`);
    }

    const openedAfter = await handle.stat({bigint: true});
    if (!sameReadState(openedBefore, openedAfter)) {
      throw new Error(`${label} ${resolved} changed while it was read`);
    }
    if (requireSafePermissions) {
      validateSafePermissions(openedAfter, resolved, label);
    }
    const pathAfter = await inspectPath(resolved, label);
    validateOpenedFile(openedAfter, pathAfter, resolved, label);
    await validateAncestors(resolved, label);

    const bytes = Buffer.concat(chunks, offset);
    if (encoding === null) {
      return bytes;
    }
    if (encoding !== 'utf8') {
      throw new TypeError(`${label} only supports null or utf8 encoding`);
    }
    try {
      return new TextDecoder('utf-8', {fatal: true}).decode(bytes);
    } catch (error) {
      throw new Error(`${label} ${resolved} must be UTF-8: ${error.message ?? error}`);
    }
  } finally {
    await handle.close();
  }
}

/**
 * Atomically replace one public OpenAPI artifact without following links.
 *
 * The complete payload is synced to a private same-directory temporary file,
 * the destination is revalidated immediately before rename, and the published
 * inode plus parent directory are synced before success is reported.
 */
export async function writeOpenApiAtomicFile(
  filePath,
  payload,
  {
    label = 'OpenAPI output',
    mode = 0o644,
  } = {},
) {
  const bytes = toBuffer(payload, label);
  if (!Number.isInteger(mode) || mode < 0 || mode > 0o777) {
    throw new TypeError(`${label} mode must be a Unix permission value`);
  }
  const resolved = path.resolve(filePath);
  await validateAncestors(resolved, label);
  await inspectReplaceTarget(resolved, label);

  const parent = path.dirname(resolved);
  const temporary = path.join(
    parent,
    `.openapi-atomic-${process.pid}-${randomBytes(12).toString('hex')}.tmp`,
  );
  const flags =
    fsConstants.O_WRONLY |
    fsConstants.O_CREAT |
    fsConstants.O_EXCL |
    (typeof fsConstants.O_NOFOLLOW === 'number' ? fsConstants.O_NOFOLLOW : 0);
  let handle;
  let temporaryExists = false;
  try {
    handle = await open(temporary, flags, 0o600);
    temporaryExists = true;
    let offset = 0;
    while (offset < bytes.length) {
      const {bytesWritten} = await handle.write(
        bytes,
        offset,
        bytes.length - offset,
        offset,
      );
      if (bytesWritten <= 0) {
        throw new Error(`${label} temporary write made no progress`);
      }
      offset += bytesWritten;
    }
    await handle.sync();
    await handle.chmod(mode);
    const staged = await handle.stat({bigint: true});
    if (
      !staged.isFile() ||
      staged.nlink !== 1n ||
      staged.size !== BigInt(bytes.length)
    ) {
      throw new Error(`${label} temporary file failed final validation`);
    }
    await handle.close();
    handle = null;

    await inspectReplaceTarget(resolved, label);
    await validateAncestors(resolved, label);
    await rename(temporary, resolved);
    temporaryExists = false;

    const published = await lstat(resolved, {bigint: true});
    if (
      published.isSymbolicLink() ||
      !published.isFile() ||
      published.nlink !== 1n ||
      published.dev !== staged.dev ||
      published.ino !== staged.ino ||
      published.size !== staged.size
    ) {
      throw new Error(`${label} ${resolved} changed during atomic publication`);
    }
    if (
      process.platform !== 'win32' &&
      (published.mode & 0o777n) !== BigInt(mode)
    ) {
      throw new Error(`${label} ${resolved} has an unexpected final mode`);
    }
    await validateAncestors(resolved, label);
    await syncDirectory(parent);
  } finally {
    if (handle) {
      await handle.close().catch(() => {});
    }
    if (temporaryExists) {
      await unlink(temporary).catch(() => {});
    }
  }
}

async function inspectPath(resolved, label) {
  let metadata;
  try {
    metadata = await lstat(resolved, {bigint: true});
  } catch (error) {
    const wrapped = new Error(
      `failed to inspect ${label} ${resolved}: ${error.message ?? error}`,
      {cause: error},
    );
    wrapped.code = error?.code;
    throw wrapped;
  }
  if (metadata.isSymbolicLink()) {
    throw new Error(`${label} ${resolved} must not be a symlink`);
  }
  if (!metadata.isFile()) {
    throw new Error(`${label} ${resolved} must be a regular file`);
  }
  if (metadata.nlink !== 1n) {
    throw new Error(`${label} ${resolved} must have exactly one hard link`);
  }
  return metadata;
}

async function inspectReplaceTarget(resolved, label) {
  let metadata;
  try {
    metadata = await lstat(resolved, {bigint: true});
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return;
    }
    throw error;
  }
  if (metadata.isSymbolicLink()) {
    throw new Error(`${label} ${resolved} must not be a symlink`);
  }
  if (!metadata.isFile()) {
    throw new Error(`${label} ${resolved} must be a regular file`);
  }
  if (metadata.nlink !== 1n) {
    throw new Error(`${label} ${resolved} must have exactly one hard link`);
  }
}

function validateOpenedFile(opened, current, resolved, label) {
  if (!opened.isFile()) {
    throw new Error(`${label} ${resolved} must be a regular file`);
  }
  if (opened.nlink !== 1n) {
    throw new Error(`${label} ${resolved} must have exactly one hard link`);
  }
  if (opened.dev !== current.dev || opened.ino !== current.ino) {
    throw new Error(`${label} ${resolved} changed while it was open`);
  }
}

function sameReadState(left, right) {
  return (
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs
  );
}

function validateSafePermissions(metadata, resolved, label) {
  if (
    process.platform !== 'win32' &&
    (metadata.mode & 0o022n) !== 0n
  ) {
    throw new Error(
      `${label} ${resolved} must not be writable by group or other users`,
    );
  }
}

async function validateAncestors(resolved, label) {
  const root = path.parse(resolved).root;
  let current = path.dirname(resolved);
  while (current && current !== root) {
    let metadata;
    try {
      metadata = await lstat(current, {bigint: true});
    } catch (error) {
      const wrapped = new Error(
        `failed to inspect ${label} parent ${current}: ${error.message ?? error}`,
        {cause: error},
      );
      wrapped.code = error?.code;
      throw wrapped;
    }
    if (metadata.isSymbolicLink()) {
      if (path.dirname(current) !== root) {
        throw new Error(`${label} parent ${current} must not be a symlink`);
      }
    } else if (!metadata.isDirectory()) {
      throw new Error(`${label} parent ${current} must be a directory`);
    }
    const next = path.dirname(current);
    if (next === current) {
      break;
    }
    current = next;
  }
}

async function syncDirectory(parent) {
  let handle;
  try {
    handle = await open(parent, fsConstants.O_RDONLY);
    await handle.sync();
  } catch (error) {
    if (
      process.platform === 'win32' &&
      ['EACCES', 'EINVAL', 'ENOTSUP', 'EPERM'].includes(error?.code)
    ) {
      return;
    }
    throw error;
  } finally {
    if (handle) {
      await handle.close().catch(() => {});
    }
  }
}

function toBuffer(value, label) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return Buffer.from(value, 'utf8');
  }
  throw new TypeError(`${label} payload must be a Buffer, Uint8Array, or string`);
}
