"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.ByteRangeLock = void 0;
/**
 * Byte-range lock record for NFSv4 LOCK operations.
 * Represents a single byte-range lock held by a lock-owner on a file.
 */
class ByteRangeLock {
    constructor(
    /**
     * Stateid associated with this lock.
     * Used by client to identify this lock in subsequent operations (LOCKU, etc.).
     */
    stateid, 
    /**
     * Absolute file system path of the locked file.
     * Used to identify which file this lock applies to.
     */
    path, 
    /**
     * Lock type - READ or WRITE lock.
     * READ locks (shared) can coexist with other READ locks.
     * WRITE locks (exclusive) conflict with all other locks.
     */
    locktype, 
    /**
     * Starting byte offset of the locked range.
     * 0-based offset from start of file.
     */
    offset, 
    /**
     * Length of the locked range in bytes.
     * Special value 0xFFFFFFFFFFFFFFFF means "to end of file".
     */
    length, 
    /**
     * Key identifying the lock-owner that holds this lock.
     * Format: `${clientid}:${base64(owner)}`.
     * Links this lock back to the owner for cleanup and conflict checking.
     */
    lockOwnerKey) {
        this.stateid = stateid;
        this.path = path;
        this.locktype = locktype;
        this.offset = offset;
        this.length = length;
        this.lockOwnerKey = lockOwnerKey;
    }
    /**
     * Check if this lock overlaps with a given byte range.
     * @param offset - Start offset to check
     * @param length - Length to check
     * @returns true if ranges overlap
     */
    overlaps(offset, length) {
        const MAX_UINT64 = BigInt('0xFFFFFFFFFFFFFFFF');
        const thisEnd = this.length === MAX_UINT64 ? MAX_UINT64 : this.offset + this.length;
        const otherEnd = length === MAX_UINT64 ? MAX_UINT64 : offset + length;
        return this.offset < otherEnd && offset < thisEnd;
    }
}
exports.ByteRangeLock = ByteRangeLock;
//# sourceMappingURL=ByteRangeLock.js.map