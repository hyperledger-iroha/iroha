import type { Nfsv4OperationCtx, Nfsv4Operations } from '../Nfsv4Operations';
import * as msg from '../../../messages';
import * as struct from '../../../structs';
import { ClientRecord } from '../ClientRecord';
import { OpenFileState } from '../OpenFileState';
import { OpenOwnerState } from '../OpenOwnerState';
import { LockOwnerState } from '../LockOwnerState';
import { ByteRangeLock } from '../ByteRangeLock';
import { LockStateid } from '../LockStateid';
import { FilesystemStats } from '../FilesystemStats';
import { FileHandleMapper } from './fh';
export interface Nfsv4OperationsNodeOpts {
    /** Node.js `fs` module. */
    fs: typeof import('node:fs');
    /**
     * Absolute path to the root directory to serve. This is some directory on the
     * host filesystem that the NFS server will use as its root.
     */
    dir: string;
    /**
     * Maximum number of confirmed clients to allow.
     * @default 1000
     */
    maxClients?: number;
    /**
     * Maximum number of pending clients to allow.
     * @default 1000
     */
    maxPendingClients?: number;
    /**
     * Optional function to provide filesystem statistics.
     * If not provided, defaults to 2TB available space and 2M available inodes.
     */
    fsStats?: () => Promise<FilesystemStats>;
}
/**
 * NFS v4 Operations implementation for Node.js `fs` filesystem.
 */
export declare class Nfsv4OperationsNode implements Nfsv4Operations {
    protected readonly fs: typeof import('node:fs');
    protected readonly promises: typeof import('node:fs')['promises'];
    protected dir: string;
    /**
     * Lease time in seconds.
     * Per RFC 7530 §9.5, this is the time a client has to renew its lease
     * before the server may reclaim its state. Default is 90 seconds.
     */
    protected readonly leaseTime: number;
    /** Confirmed clients. */
    protected clients: Map<bigint, ClientRecord>;
    /** Clients pending SETCLIENTID_CONFIRM confirmation. */
    protected pendingClients: Map<bigint, ClientRecord>;
    /** Maximum number of client records to keep. */
    protected maxClients: number;
    /** Maximum number of pending client records to keep. */
    protected maxPendingClients: number;
    /** Next client ID to assign. */
    protected nextClientId: bigint;
    /** Boot stamp, identifies server instance, 16 bits. */
    protected bootStamp: number;
    protected readonly fh: FileHandleMapper;
    /** Next stateid sequence number. */
    protected nextStateidSeqid: number;
    /** Map from stateid (as string key) to open file state. */
    protected openFiles: Map<string, OpenFileState>;
    /** Map from open-owner key to owner state. */
    protected openOwners: Map<string, OpenOwnerState>;
    /** Map from lock key to byte-range lock. */
    protected locks: Map<string, ByteRangeLock>;
    /** Map from lock-owner key to lock-owner state. */
    protected lockOwners: Map<string, LockOwnerState>;
    /** Map from lock stateid 'other' field to lock stateid state. Per RFC 7530, one stateid per lock-owner per file. */
    protected lockStateids: Map<string, LockStateid>;
    /**
     * Server-wide monotonic change counter for directory change_info.
     * Incremented on every mutating operation (RENAME, REMOVE, CREATE, etc.).
     * Used to populate change_info4 before/after values for client cache validation.
     */
    protected changeCounter: bigint;
    /**
     * Function to retrieve filesystem statistics.
     */
    protected fsStats: () => Promise<FilesystemStats>;
    constructor(opts: Nfsv4OperationsNodeOpts);
    /**
     * Default filesystem statistics: 2TB available space, 2M available inodes.
     */
    protected defaultFsStats: () => Promise<FilesystemStats>;
    protected findClientByIdString(map: Map<bigint, ClientRecord>, clientIdString: Uint8Array): [bigint, ClientRecord] | undefined;
    protected enforceClientLimit(): void;
    protected enforcePendingClientLimit(): void;
    protected makeOpenOwnerKey(clientid: bigint, owner: Uint8Array): string;
    /**
     * Validates a seqid from a client request against the owner's current seqid.
     * Per RFC 7530 §9.1.7, the server expects seqid = last_seqid + 1 for new operations,
     * or seqid = last_seqid for replayed requests (idempotent retry).
     *
     * @param requestSeqid - seqid from the client request
     * @param ownerSeqid - current seqid stored for the owner
     * @returns 'valid' if seqid matches expected next value, 'replay' if it matches last value, 'invalid' otherwise
     */
    protected validateSeqid(requestSeqid: number, ownerSeqid: number): 'valid' | 'replay' | 'invalid';
    /**
     * Renews the lease for a client.
     * Per RFC 7530 §9.5, any stateful operation renews the client's lease.
     *
     * @param clientid - The client ID whose lease should be renewed
     */
    protected renewClientLease(clientid: bigint): void;
    protected makeStateidKey(stateid: struct.Nfsv4Stateid): string;
    protected createStateid(): struct.Nfsv4Stateid;
    protected canAccessFile(path: string, shareAccess: number, shareDeny: number): boolean;
    protected makeLockOwnerKey(clientid: bigint, owner: Uint8Array): string;
    protected makeOpenRequestKey(ownerKey: string, currentPath: string, request: msg.Nfsv4OpenRequest): string;
    protected makeLockRequestKey(lockOwnerKey: string, filePath: string, locktype: number, offset: bigint, length: bigint, seqid: number): string;
    protected makeLockuRequestKey(lockOwnerKey: string, stateid: struct.Nfsv4Stateid, offset: bigint, length: bigint, seqid: number): string;
    protected makeLockKey(stateid: struct.Nfsv4Stateid, offset: bigint, length: bigint): string;
    protected makeLockStateidKey(lockOwnerKey: string, path: string): string;
    protected getOrCreateLockStateid(lockOwnerKey: string, path: string): LockStateid;
    protected findLockStateidByOther(other: Uint8Array): LockStateid | undefined;
    protected hasConflictingLock(path: string, locktype: number, offset: bigint, length: bigint, ownerKey: string): boolean;
    /**
     * Establishes client ID or updates callback information.
     * Returns a client ID and confirmation verifier for SETCLIENTID_CONFIRM.
     */
    SETCLIENTID(request: msg.Nfsv4SetclientidRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4SetclientidResponse>;
    /**
     * Confirms a client ID established by SETCLIENTID.
     * Transitions unconfirmed client record to confirmed state.
     */
    SETCLIENTID_CONFIRM(request: msg.Nfsv4SetclientidConfirmRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4SetclientidConfirmResponse>;
    ILLEGAL(request: msg.Nfsv4IllegalRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4IllegalResponse>;
    PUTROOTFH(request: msg.Nfsv4PutrootfhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4PutrootfhResponse>;
    PUTPUBFH(request: msg.Nfsv4PutpubfhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4PutpubfhResponse>;
    PUTFH(request: msg.Nfsv4PutfhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4PutfhResponse>;
    GETFH(request: msg.Nfsv4GetfhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4GetfhResponse>;
    RESTOREFH(request: msg.Nfsv4RestorefhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4RestorefhResponse>;
    SAVEFH(request: msg.Nfsv4SavefhRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4SavefhResponse>;
    private absolutePath;
    LOOKUP(request: msg.Nfsv4LookupRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LookupResponse>;
    LOOKUPP(request: msg.Nfsv4LookuppRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LookuppResponse>;
    GETATTR(request: msg.Nfsv4GetattrRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4GetattrResponse>;
    ACCESS(request: msg.Nfsv4AccessRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4AccessResponse>;
    READDIR(request: msg.Nfsv4ReaddirRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4ReaddirResponse>;
    OPEN(request: msg.Nfsv4OpenRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4OpenResponse>;
    OPENATTR(request: msg.Nfsv4OpenattrRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4OpenattrResponse>;
    OPEN_CONFIRM(request: msg.Nfsv4OpenConfirmRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4OpenConfirmResponse>;
    OPEN_DOWNGRADE(request: msg.Nfsv4OpenDowngradeRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4OpenDowngradeResponse>;
    CLOSE(request: msg.Nfsv4CloseRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4CloseResponse>;
    SECINFO(request: msg.Nfsv4SecinfoRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4SecinfoResponse>;
    LOCK(request: msg.Nfsv4LockRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LockResponse>;
    LOCKT(request: msg.Nfsv4LocktRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LocktResponse>;
    LOCKU(request: msg.Nfsv4LockuRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LockuResponse>;
    RELEASE_LOCKOWNER(request: msg.Nfsv4ReleaseLockOwnerRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4ReleaseLockOwnerResponse>;
    RENEW(request: msg.Nfsv4RenewRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4RenewResponse>;
    READ(request: msg.Nfsv4ReadRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4ReadResponse>;
    READLINK(request: msg.Nfsv4ReadlinkRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4ReadlinkResponse>;
    REMOVE(request: msg.Nfsv4RemoveRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4RemoveResponse>;
    RENAME(request: msg.Nfsv4RenameRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4RenameResponse>;
    WRITE(request: msg.Nfsv4WriteRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4WriteResponse>;
    DELEGPURGE(request: msg.Nfsv4DelegpurgeRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4DelegpurgeResponse>;
    DELEGRETURN(request: msg.Nfsv4DelegreturnRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4DelegreturnResponse>;
    COMMIT(request: msg.Nfsv4CommitRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4CommitResponse>;
    CREATE(request: msg.Nfsv4CreateRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4CreateResponse>;
    LINK(request: msg.Nfsv4LinkRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4LinkResponse>;
    NVERIFY(request: msg.Nfsv4NverifyRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4NverifyResponse>;
    SETATTR(request: msg.Nfsv4SetattrRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4SetattrResponse>;
    VERIFY(request: msg.Nfsv4VerifyRequest, ctx: Nfsv4OperationCtx): Promise<msg.Nfsv4VerifyResponse>;
}
