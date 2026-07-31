"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.LockStateid = void 0;
const tslib_1 = require("tslib");
const struct = tslib_1.__importStar(require("../../structs"));
/**
 * Lock stateid record for NFSv4 lock operations.
 * Per RFC 7530 §9.1.4.1, all locks held on a particular file by a particular
 * owner share a single stateid, with the seqid incremented on each LOCK/LOCKU.
 * The stateid remains valid even after all locks are freed, as long as the
 * associated open file remains open.
 */
class LockStateid {
    constructor(
    /**
     * The "other" field of the stateid (96 bits).
     * Uniquely identifies this lock-owner+file combination.
     * Remains constant across all LOCK/LOCKU operations.
     */
    other, 
    /**
     * Current seqid value for this lock stateid.
     * Incremented on each LOCK or LOCKU operation that affects locks.
     * Starts at 1 when first created.
     */
    seqid, 
    /**
     * Key identifying the lock-owner that owns this stateid.
     * Format: `${clientid}:${hex(owner)}`.
     */
    lockOwnerKey, 
    /**
     * Absolute file system path of the file this stateid applies to.
     * A lock-owner can have different stateids for different files.
     */
    path) {
        this.other = other;
        this.seqid = seqid;
        this.lockOwnerKey = lockOwnerKey;
        this.path = path;
    }
    /**
     * Get the full stateid with current seqid.
     */
    toStateid() {
        return new struct.Nfsv4Stateid(this.seqid, this.other);
    }
    /**
     * Increment seqid and return new stateid.
     * Per RFC 7530, seqid wraps from 0xFFFFFFFF to 1 (not 0).
     */
    incrementAndGetStateid() {
        this.seqid = this.seqid === 0xffffffff ? 1 : this.seqid + 1;
        return this.toStateid();
    }
}
exports.LockStateid = LockStateid;
//# sourceMappingURL=LockStateid.js.map