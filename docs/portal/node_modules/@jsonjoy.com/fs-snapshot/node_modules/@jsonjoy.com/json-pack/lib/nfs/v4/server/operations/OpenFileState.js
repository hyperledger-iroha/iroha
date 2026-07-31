"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.OpenFileState = void 0;
/**
 * Open file state record for NFSv4 OPEN operations.
 * Tracks state for an individual file that has been opened by a client,
 * including the stateid, file descriptor, share reservations, and confirmation status.
 */
class OpenFileState {
    constructor(
    /**
     * Stateid assigned to this open file.
     * Used by client to identify this particular open in subsequent operations
     * (CLOSE, OPEN_DOWNGRADE, READ, WRITE, etc.).
     */
    stateid, 
    /**
     * Absolute file system path of the opened file.
     * Used to identify the file and check for share reservation conflicts.
     */
    path, 
    /**
     * Node.js file descriptor/handle for the opened file.
     * Used to perform I/O operations and must be closed when the file is closed.
     */
    fd, 
    /**
     * Share access mode - which operations this open allows.
     * Bitwise OR of OPEN4_SHARE_ACCESS_READ, OPEN4_SHARE_ACCESS_WRITE.
     * Controls what the opener can do with the file.
     */
    shareAccess, 
    /**
     * Share deny mode - which operations this open denies to others.
     * Bitwise OR of OPEN4_SHARE_DENY_READ, OPEN4_SHARE_DENY_WRITE, or OPEN4_SHARE_DENY_NONE.
     * Controls what conflicting operations are blocked for other opens.
     */
    shareDeny, 
    /**
     * Key identifying the open-owner that opened this file.
     * Format: `${clientid}:${base64(owner)}`.
     * Links this open back to the owner for sequence number management.
     */
    openOwnerKey, 
    /**
     * Sequence number for this open.
     * Used to detect replayed or out-of-order operations.
     * Incremented on OPEN_CONFIRM and OPEN_DOWNGRADE.
     */
    seqid, 
    /**
     * Whether this open has been confirmed via OPEN_CONFIRM.
     * NFSv4.0 requires new opens from new open-owners to be confirmed.
     * Once confirmed, the open can be used for READ/WRITE operations.
     */
    confirmed) {
        this.stateid = stateid;
        this.path = path;
        this.fd = fd;
        this.shareAccess = shareAccess;
        this.shareDeny = shareDeny;
        this.openOwnerKey = openOwnerKey;
        this.seqid = seqid;
        this.confirmed = confirmed;
    }
}
exports.OpenFileState = OpenFileState;
//# sourceMappingURL=OpenFileState.js.map