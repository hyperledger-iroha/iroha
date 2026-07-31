/**
 * NFSv4 attribute metadata and classification.
 * Based on RFC 7530 Section 5.
 */
import { Nfsv4Attr } from './constants';
/**
 * Per-server attributes (Section 5.4).
 * These attributes are global to the entire server.
 */
export declare const PER_SERVER_ATTRS: Set<Nfsv4Attr>;
/**
 * Per-file system attributes (Section 5.4).
 * These attributes are consistent across all objects within a given file system.
 */
export declare const PER_FS_ATTRS: Set<Nfsv4Attr>;
/**
 * Attributes that must be the same for all objects within a file system (Section 5.4).
 * These are always homogeneous.
 */
export declare const HOMOGENEOUS_ATTRS: Set<Nfsv4Attr>;
/**
 * Read-only (get-only) attributes (Section 5.5).
 * Can be retrieved via GETATTR but not set via SETATTR.
 * Attempting to set these returns NFS4ERR_INVAL.
 */
export declare const GET_ONLY_ATTRS: Set<Nfsv4Attr>;
/**
 * Write-only (set-only) attributes (Section 5.5).
 * Can be set via SETATTR but not retrieved via GETATTR.
 * Attempting to get these returns NFS4ERR_INVAL.
 */
export declare const SET_ONLY_ATTRS: Set<Nfsv4Attr>;
/**
 * REQUIRED attributes (Section 5.6, Table 3).
 * Server MUST support these attributes.
 */
export declare const REQUIRED_ATTRS: Set<Nfsv4Attr>;
/**
 * RECOMMENDED attributes (Section 5.7, Table 4).
 * Server SHOULD support these attributes.
 */
export declare const RECOMMENDED_ATTRS: Set<Nfsv4Attr>;
/**
 * Attributes that require fs.Stats (lstat) to compute.
 * If none of these are requested, we can skip the lstat call.
 */
export declare const STAT_ATTRS: Set<Nfsv4Attr>;
/**
 * Attributes that require filesystem stats (e.g. disk space).
 * If none of these are requested, we can skip the filesystem stats call.
 */
export declare const FS_ATTRS: Set<Nfsv4Attr>;
/**
 * Extract attribute numbers from a bitmap mask.
 *
 * @todo PERF: More efficient would be to parse to `Array<number>` and
 *     also use `Array<number>` for {@link overlap} calculation.
 */
export declare const parseBitmask: (mask: number[]) => Set<number>;
/**
 * Check if two sets overlap (have any elements in common).
 */
export declare const overlaps: <T>(a: Set<T>, b: Set<T>) => boolean;
/**
 * Check if attempting to get a set-only attribute (returns NFS4ERR_INVAL).
 */
export declare const containsSetOnlyAttr: (requestedAttrs: Set<number>) => boolean;
/**
 * Check if any requested attributes require lstat.
 */
export declare const requiresLstat: (requestedAttrs: Set<number>) => boolean;
export declare const requiresFsStats: (requestedAttrs: Set<number>) => boolean;
export declare const setBit: (mask: number[], attrNum: Nfsv4Attr) => void;
/**
 * Helper to convert attribute numbers to bitmap array.
 * @param attrNums - Array of attribute numbers (Nfsv4Attr values)
 * @returns Bitmap array suitable for Nfsv4Bitmap constructor
 */
export declare const attrNumsToBitmap: (attrNums: Nfsv4Attr[]) => number[];
