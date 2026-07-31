"use strict";
/**
 * NFSv4 attribute metadata and classification.
 * Based on RFC 7530 Section 5.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.attrNumsToBitmap = exports.setBit = exports.requiresFsStats = exports.requiresLstat = exports.containsSetOnlyAttr = exports.overlaps = exports.parseBitmask = exports.FS_ATTRS = exports.STAT_ATTRS = exports.RECOMMENDED_ATTRS = exports.REQUIRED_ATTRS = exports.SET_ONLY_ATTRS = exports.GET_ONLY_ATTRS = exports.HOMOGENEOUS_ATTRS = exports.PER_FS_ATTRS = exports.PER_SERVER_ATTRS = void 0;
/**
 * Per-server attributes (Section 5.4).
 * These attributes are global to the entire server.
 */
exports.PER_SERVER_ATTRS = new Set([10 /* Nfsv4Attr.FATTR4_LEASE_TIME */]);
/**
 * Per-file system attributes (Section 5.4).
 * These attributes are consistent across all objects within a given file system.
 */
exports.PER_FS_ATTRS = new Set([
    0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */,
    2 /* Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */,
    5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */,
    6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */,
    9 /* Nfsv4Attr.FATTR4_UNIQUE_HANDLES */,
    13 /* Nfsv4Attr.FATTR4_ACLSUPPORT */,
    15 /* Nfsv4Attr.FATTR4_CANSETTIME */,
    16 /* Nfsv4Attr.FATTR4_CASE_INSENSITIVE */,
    17 /* Nfsv4Attr.FATTR4_CASE_PRESERVING */,
    18 /* Nfsv4Attr.FATTR4_CHOWN_RESTRICTED */,
    21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */,
    22 /* Nfsv4Attr.FATTR4_FILES_FREE */,
    23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */,
    24 /* Nfsv4Attr.FATTR4_FS_LOCATIONS */,
    26 /* Nfsv4Attr.FATTR4_HOMOGENEOUS */,
    27 /* Nfsv4Attr.FATTR4_MAXFILESIZE */,
    29 /* Nfsv4Attr.FATTR4_MAXNAME */,
    30 /* Nfsv4Attr.FATTR4_MAXREAD */,
    31 /* Nfsv4Attr.FATTR4_MAXWRITE */,
    34 /* Nfsv4Attr.FATTR4_NO_TRUNC */,
    42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */,
    43 /* Nfsv4Attr.FATTR4_SPACE_FREE */,
    44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */,
    51 /* Nfsv4Attr.FATTR4_TIME_DELTA */,
]);
/**
 * Attributes that must be the same for all objects within a file system (Section 5.4).
 * These are always homogeneous.
 */
exports.HOMOGENEOUS_ATTRS = new Set([
    0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */,
    8 /* Nfsv4Attr.FATTR4_FSID */,
    26 /* Nfsv4Attr.FATTR4_HOMOGENEOUS */,
    5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */,
    6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */,
]);
/**
 * Read-only (get-only) attributes (Section 5.5).
 * Can be retrieved via GETATTR but not set via SETATTR.
 * Attempting to set these returns NFS4ERR_INVAL.
 */
exports.GET_ONLY_ATTRS = new Set([
    0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */,
    1 /* Nfsv4Attr.FATTR4_TYPE */,
    2 /* Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */,
    3 /* Nfsv4Attr.FATTR4_CHANGE */,
    5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */,
    6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */,
    7 /* Nfsv4Attr.FATTR4_NAMED_ATTR */,
    8 /* Nfsv4Attr.FATTR4_FSID */,
    9 /* Nfsv4Attr.FATTR4_UNIQUE_HANDLES */,
    10 /* Nfsv4Attr.FATTR4_LEASE_TIME */,
    11 /* Nfsv4Attr.FATTR4_RDATTR_ERROR */,
    19 /* Nfsv4Attr.FATTR4_FILEHANDLE */,
    13 /* Nfsv4Attr.FATTR4_ACLSUPPORT */,
    15 /* Nfsv4Attr.FATTR4_CANSETTIME */,
    16 /* Nfsv4Attr.FATTR4_CASE_INSENSITIVE */,
    17 /* Nfsv4Attr.FATTR4_CASE_PRESERVING */,
    18 /* Nfsv4Attr.FATTR4_CHOWN_RESTRICTED */,
    20 /* Nfsv4Attr.FATTR4_FILEID */,
    21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */,
    22 /* Nfsv4Attr.FATTR4_FILES_FREE */,
    23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */,
    24 /* Nfsv4Attr.FATTR4_FS_LOCATIONS */,
    26 /* Nfsv4Attr.FATTR4_HOMOGENEOUS */,
    27 /* Nfsv4Attr.FATTR4_MAXFILESIZE */,
    28 /* Nfsv4Attr.FATTR4_MAXLINK */,
    29 /* Nfsv4Attr.FATTR4_MAXNAME */,
    30 /* Nfsv4Attr.FATTR4_MAXREAD */,
    31 /* Nfsv4Attr.FATTR4_MAXWRITE */,
    55 /* Nfsv4Attr.FATTR4_MOUNTED_ON_FILEID */,
    34 /* Nfsv4Attr.FATTR4_NO_TRUNC */,
    35 /* Nfsv4Attr.FATTR4_NUMLINKS */,
    38 /* Nfsv4Attr.FATTR4_QUOTA_AVAIL_HARD */,
    39 /* Nfsv4Attr.FATTR4_QUOTA_AVAIL_SOFT */,
    40 /* Nfsv4Attr.FATTR4_QUOTA_USED */,
    41 /* Nfsv4Attr.FATTR4_RAWDEV */,
    42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */,
    43 /* Nfsv4Attr.FATTR4_SPACE_FREE */,
    44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */,
    45 /* Nfsv4Attr.FATTR4_SPACE_USED */,
    47 /* Nfsv4Attr.FATTR4_TIME_ACCESS */,
    51 /* Nfsv4Attr.FATTR4_TIME_DELTA */,
    52 /* Nfsv4Attr.FATTR4_TIME_METADATA */,
    53 /* Nfsv4Attr.FATTR4_TIME_MODIFY */,
]);
/**
 * Write-only (set-only) attributes (Section 5.5).
 * Can be set via SETATTR but not retrieved via GETATTR.
 * Attempting to get these returns NFS4ERR_INVAL.
 */
exports.SET_ONLY_ATTRS = new Set([48 /* Nfsv4Attr.FATTR4_TIME_ACCESS_SET */, 54 /* Nfsv4Attr.FATTR4_TIME_MODIFY_SET */]);
/**
 * REQUIRED attributes (Section 5.6, Table 3).
 * Server MUST support these attributes.
 */
exports.REQUIRED_ATTRS = new Set([
    0 /* Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */,
    1 /* Nfsv4Attr.FATTR4_TYPE */,
    2 /* Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */,
    3 /* Nfsv4Attr.FATTR4_CHANGE */,
    4 /* Nfsv4Attr.FATTR4_SIZE */,
    5 /* Nfsv4Attr.FATTR4_LINK_SUPPORT */,
    6 /* Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */,
    7 /* Nfsv4Attr.FATTR4_NAMED_ATTR */,
    8 /* Nfsv4Attr.FATTR4_FSID */,
    9 /* Nfsv4Attr.FATTR4_UNIQUE_HANDLES */,
    10 /* Nfsv4Attr.FATTR4_LEASE_TIME */,
    11 /* Nfsv4Attr.FATTR4_RDATTR_ERROR */,
    19 /* Nfsv4Attr.FATTR4_FILEHANDLE */,
]);
/**
 * RECOMMENDED attributes (Section 5.7, Table 4).
 * Server SHOULD support these attributes.
 */
exports.RECOMMENDED_ATTRS = new Set([
    12 /* Nfsv4Attr.FATTR4_ACL */,
    13 /* Nfsv4Attr.FATTR4_ACLSUPPORT */,
    14 /* Nfsv4Attr.FATTR4_ARCHIVE */,
    15 /* Nfsv4Attr.FATTR4_CANSETTIME */,
    16 /* Nfsv4Attr.FATTR4_CASE_INSENSITIVE */,
    17 /* Nfsv4Attr.FATTR4_CASE_PRESERVING */,
    18 /* Nfsv4Attr.FATTR4_CHOWN_RESTRICTED */,
    20 /* Nfsv4Attr.FATTR4_FILEID */,
    21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */,
    22 /* Nfsv4Attr.FATTR4_FILES_FREE */,
    23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */,
    24 /* Nfsv4Attr.FATTR4_FS_LOCATIONS */,
    25 /* Nfsv4Attr.FATTR4_HIDDEN */,
    26 /* Nfsv4Attr.FATTR4_HOMOGENEOUS */,
    27 /* Nfsv4Attr.FATTR4_MAXFILESIZE */,
    28 /* Nfsv4Attr.FATTR4_MAXLINK */,
    29 /* Nfsv4Attr.FATTR4_MAXNAME */,
    30 /* Nfsv4Attr.FATTR4_MAXREAD */,
    31 /* Nfsv4Attr.FATTR4_MAXWRITE */,
    32 /* Nfsv4Attr.FATTR4_MIMETYPE */,
    33 /* Nfsv4Attr.FATTR4_MODE */,
    55 /* Nfsv4Attr.FATTR4_MOUNTED_ON_FILEID */,
    34 /* Nfsv4Attr.FATTR4_NO_TRUNC */,
    35 /* Nfsv4Attr.FATTR4_NUMLINKS */,
    36 /* Nfsv4Attr.FATTR4_OWNER */,
    37 /* Nfsv4Attr.FATTR4_OWNER_GROUP */,
    38 /* Nfsv4Attr.FATTR4_QUOTA_AVAIL_HARD */,
    39 /* Nfsv4Attr.FATTR4_QUOTA_AVAIL_SOFT */,
    40 /* Nfsv4Attr.FATTR4_QUOTA_USED */,
    41 /* Nfsv4Attr.FATTR4_RAWDEV */,
    42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */,
    43 /* Nfsv4Attr.FATTR4_SPACE_FREE */,
    44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */,
    45 /* Nfsv4Attr.FATTR4_SPACE_USED */,
    46 /* Nfsv4Attr.FATTR4_SYSTEM */,
    47 /* Nfsv4Attr.FATTR4_TIME_ACCESS */,
    48 /* Nfsv4Attr.FATTR4_TIME_ACCESS_SET */,
    49 /* Nfsv4Attr.FATTR4_TIME_BACKUP */,
    50 /* Nfsv4Attr.FATTR4_TIME_CREATE */,
    51 /* Nfsv4Attr.FATTR4_TIME_DELTA */,
    52 /* Nfsv4Attr.FATTR4_TIME_METADATA */,
    53 /* Nfsv4Attr.FATTR4_TIME_MODIFY */,
    54 /* Nfsv4Attr.FATTR4_TIME_MODIFY_SET */,
]);
/**
 * Attributes that require fs.Stats (lstat) to compute.
 * If none of these are requested, we can skip the lstat call.
 */
exports.STAT_ATTRS = new Set([
    1 /* Nfsv4Attr.FATTR4_TYPE */,
    3 /* Nfsv4Attr.FATTR4_CHANGE */,
    4 /* Nfsv4Attr.FATTR4_SIZE */,
    20 /* Nfsv4Attr.FATTR4_FILEID */,
    33 /* Nfsv4Attr.FATTR4_MODE */,
    35 /* Nfsv4Attr.FATTR4_NUMLINKS */,
    41 /* Nfsv4Attr.FATTR4_RAWDEV */,
    45 /* Nfsv4Attr.FATTR4_SPACE_USED */,
    47 /* Nfsv4Attr.FATTR4_TIME_ACCESS */,
    52 /* Nfsv4Attr.FATTR4_TIME_METADATA */,
    53 /* Nfsv4Attr.FATTR4_TIME_MODIFY */,
]);
/**
 * Attributes that require filesystem stats (e.g. disk space).
 * If none of these are requested, we can skip the filesystem stats call.
 */
exports.FS_ATTRS = new Set([
    21 /* Nfsv4Attr.FATTR4_FILES_AVAIL */,
    22 /* Nfsv4Attr.FATTR4_FILES_FREE */,
    23 /* Nfsv4Attr.FATTR4_FILES_TOTAL */,
    42 /* Nfsv4Attr.FATTR4_SPACE_AVAIL */,
    43 /* Nfsv4Attr.FATTR4_SPACE_FREE */,
    44 /* Nfsv4Attr.FATTR4_SPACE_TOTAL */,
]);
/**
 * Extract attribute numbers from a bitmap mask.
 *
 * @todo PERF: More efficient would be to parse to `Array<number>` and
 *     also use `Array<number>` for {@link overlap} calculation.
 */
const parseBitmask = (mask) => {
    const attrs = new Set();
    const length = mask.length;
    for (let i = 0, word = mask[0], base = 0; i < length; i++, word = mask[i], base = i * 32)
        for (let bit = 0; word; bit++, word >>>= 1)
            if (word & 1)
                attrs.add(base + bit);
    return attrs;
};
exports.parseBitmask = parseBitmask;
/**
 * Check if two sets overlap (have any elements in common).
 */
const overlaps = (a, b) => {
    for (const element of b)
        if (a.has(element))
            return true;
    return false;
};
exports.overlaps = overlaps;
/**
 * Check if attempting to get a set-only attribute (returns NFS4ERR_INVAL).
 */
const containsSetOnlyAttr = (requestedAttrs) => (0, exports.overlaps)(requestedAttrs, exports.SET_ONLY_ATTRS);
exports.containsSetOnlyAttr = containsSetOnlyAttr;
/**
 * Check if any requested attributes require lstat.
 */
const requiresLstat = (requestedAttrs) => (0, exports.overlaps)(requestedAttrs, exports.STAT_ATTRS);
exports.requiresLstat = requiresLstat;
const requiresFsStats = (requestedAttrs) => (0, exports.overlaps)(requestedAttrs, exports.FS_ATTRS);
exports.requiresFsStats = requiresFsStats;
const setBit = (mask, attrNum) => {
    const wordIndex = Math.floor(attrNum / 32);
    const bitIndex = attrNum % 32;
    while (mask.length <= wordIndex)
        mask.push(0);
    mask[wordIndex] |= 1 << bitIndex;
};
exports.setBit = setBit;
/**
 * Helper to convert attribute numbers to bitmap array.
 * @param attrNums - Array of attribute numbers (Nfsv4Attr values)
 * @returns Bitmap array suitable for Nfsv4Bitmap constructor
 */
const attrNumsToBitmap = (attrNums) => {
    const mask = [];
    for (const attrNum of attrNums)
        (0, exports.setBit)(mask, attrNum);
    return mask;
};
exports.attrNumsToBitmap = attrNumsToBitmap;
//# sourceMappingURL=attributes.js.map