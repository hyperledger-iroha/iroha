import type * as struct from '../../structs';
/**
 * Byte-range lock record for NFSv4 LOCK operations.
 * Represents a single byte-range lock held by a lock-owner on a file.
 */
export declare class ByteRangeLock {
    /**
     * Stateid associated with this lock.
     * Used by client to identify this lock in subsequent operations (LOCKU, etc.).
     */
    readonly stateid: struct.Nfsv4Stateid;
    /**
     * Absolute file system path of the locked file.
     * Used to identify which file this lock applies to.
     */
    readonly path: string;
    /**
     * Lock type - READ or WRITE lock.
     * READ locks (shared) can coexist with other READ locks.
     * WRITE locks (exclusive) conflict with all other locks.
     */
    readonly locktype: number;
    /**
     * Starting byte offset of the locked range.
     * 0-based offset from start of file.
     */
    readonly offset: bigint;
    /**
     * Length of the locked range in bytes.
     * Special value 0xFFFFFFFFFFFFFFFF means "to end of file".
     */
    readonly length: bigint;
    /**
     * Key identifying the lock-owner that holds this lock.
     * Format: `${clientid}:${base64(owner)}`.
     * Links this lock back to the owner for cleanup and conflict checking.
     */
    readonly lockOwnerKey: string;
    constructor(
    /**
     * Stateid associated with this lock.
     * Used by client to identify this lock in subsequent operations (LOCKU, etc.).
     */
    stateid: struct.Nfsv4Stateid, 
    /**
     * Absolute file system path of the locked file.
     * Used to identify which file this lock applies to.
     */
    path: string, 
    /**
     * Lock type - READ or WRITE lock.
     * READ locks (shared) can coexist with other READ locks.
     * WRITE locks (exclusive) conflict with all other locks.
     */
    locktype: number, 
    /**
     * Starting byte offset of the locked range.
     * 0-based offset from start of file.
     */
    offset: bigint, 
    /**
     * Length of the locked range in bytes.
     * Special value 0xFFFFFFFFFFFFFFFF means "to end of file".
     */
    length: bigint, 
    /**
     * Key identifying the lock-owner that holds this lock.
     * Format: `${clientid}:${base64(owner)}`.
     * Links this lock back to the owner for cleanup and conflict checking.
     */
    lockOwnerKey: string);
    /**
     * Check if this lock overlaps with a given byte range.
     * @param offset - Start offset to check
     * @param length - Length to check
     * @returns true if ranges overlap
     */
    overlaps(offset: bigint, length: bigint): boolean;
}
