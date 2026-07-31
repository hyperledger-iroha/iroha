import * as struct from '../../structs';
/**
 * Lock stateid record for NFSv4 lock operations.
 * Per RFC 7530 §9.1.4.1, all locks held on a particular file by a particular
 * owner share a single stateid, with the seqid incremented on each LOCK/LOCKU.
 * The stateid remains valid even after all locks are freed, as long as the
 * associated open file remains open.
 */
export declare class LockStateid {
    /**
     * The "other" field of the stateid (96 bits).
     * Uniquely identifies this lock-owner+file combination.
     * Remains constant across all LOCK/LOCKU operations.
     */
    readonly other: Uint8Array;
    /**
     * Current seqid value for this lock stateid.
     * Incremented on each LOCK or LOCKU operation that affects locks.
     * Starts at 1 when first created.
     */
    seqid: number;
    /**
     * Key identifying the lock-owner that owns this stateid.
     * Format: `${clientid}:${hex(owner)}`.
     */
    readonly lockOwnerKey: string;
    /**
     * Absolute file system path of the file this stateid applies to.
     * A lock-owner can have different stateids for different files.
     */
    readonly path: string;
    constructor(
    /**
     * The "other" field of the stateid (96 bits).
     * Uniquely identifies this lock-owner+file combination.
     * Remains constant across all LOCK/LOCKU operations.
     */
    other: Uint8Array, 
    /**
     * Current seqid value for this lock stateid.
     * Incremented on each LOCK or LOCKU operation that affects locks.
     * Starts at 1 when first created.
     */
    seqid: number, 
    /**
     * Key identifying the lock-owner that owns this stateid.
     * Format: `${clientid}:${hex(owner)}`.
     */
    lockOwnerKey: string, 
    /**
     * Absolute file system path of the file this stateid applies to.
     * A lock-owner can have different stateids for different files.
     */
    path: string);
    /**
     * Get the full stateid with current seqid.
     */
    toStateid(): struct.Nfsv4Stateid;
    /**
     * Increment seqid and return new stateid.
     * Per RFC 7530, seqid wraps from 0xFFFFFFFF to 1 (not 0).
     */
    incrementAndGetStateid(): struct.Nfsv4Stateid;
}
