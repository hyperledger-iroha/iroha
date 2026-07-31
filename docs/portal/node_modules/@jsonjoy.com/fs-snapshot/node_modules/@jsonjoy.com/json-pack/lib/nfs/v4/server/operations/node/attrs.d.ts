/**
 * Attribute encoding utilities for NFSv4 server operations.
 */
import type { Stats } from 'node:fs';
import * as struct from '../../../structs';
import type { FilesystemStats } from '../FilesystemStats';
/**
 * Encodes file attributes based on the requested bitmap.
 * Returns the attributes as a Nfsv4Fattr structure.
 * @param requestedAttrs Bitmap of requested attributes
 * @param stats Optional file stats (required only if stat-based attributes are requested)
 * @param path File path (for context)
 * @param fh Optional file handle (required only if FATTR4_FILEHANDLE is requested)
 * @param leaseTime Optional lease time in seconds (required only if FATTR4_LEASE_TIME is requested)
 * @param fsStats Optional filesystem statistics (required for space/files attributes)
 */
export declare const encodeAttrs: (requestedAttrs: struct.Nfsv4Bitmap, stats: Stats | undefined, path: string, fh?: Uint8Array, leaseTime?: number, fsStats?: FilesystemStats) => struct.Nfsv4Fattr;
