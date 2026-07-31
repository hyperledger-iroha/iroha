import * as msg from './messages';
import * as structs from './structs';
import { type Nfsv4LockType } from './constants';
/**
 * Static builder helpers for NFS v4 operations.
 * Provides a simpler API for constructing NFS v4 request messages.
 *
 * @example
 * ```ts
 * const response = await client.compound([
 *   nfs.PUTROOTFH(),
 *   nfs.LOOKUP('file.txt'),
 *   nfs.GETATTR([0x00000001]),
 * ]);
 * ```
 */
export declare const nfs: {
    /**
     * PUTROOTFH - Set current filehandle to root of export.
     */
    PUTROOTFH(): msg.Nfsv4PutrootfhRequest;
    /**
     * PUTFH - Set current filehandle.
     * @param fh - Filehandle to set as current
     */
    PUTFH(fh: structs.Nfsv4Fh): msg.Nfsv4PutfhRequest;
    /**
     * PUTPUBFH - Set current filehandle to public filehandle.
     */
    PUTPUBFH(): msg.Nfsv4PutpubfhRequest;
    /**
     * GETFH - Get current filehandle.
     */
    GETFH(): msg.Nfsv4GetfhRequest;
    /**
     * LOOKUP - Lookup filename in current directory.
     * @param name - Filename to lookup
     */
    LOOKUP(name: string): msg.Nfsv4LookupRequest;
    /**
     * LOOKUPP - Lookup parent directory (..).
     */
    LOOKUPP(): msg.Nfsv4LookuppRequest;
    /**
     * GETATTR - Get file attributes.
     * @param attrBitmap - Attribute bitmap (array of uint32 values)
     */
    GETATTR(attrBitmap: number[]): msg.Nfsv4GetattrRequest;
    /**
     * READDIR - Read directory entries.
     * @param attrBitmap - Attribute bitmap for entries (single uint32 or array)
     * @param cookieverf - Cookie verifier (8 bytes), defaults to zeros
     * @param cookie - Starting cookie, defaults to 0
     * @param dircount - Max bytes for directory info, defaults to 1000
     * @param maxcount - Max bytes for reply, defaults to 8192
     */
    READDIR(attrBitmap: number | number[], cookieverf?: Uint8Array, cookie?: bigint, dircount?: number, maxcount?: number): msg.Nfsv4ReaddirRequest;
    /**
     * ACCESS - Check access permissions.
     * @param accessMask - Access mask (default: 0x3f for all bits)
     */
    ACCESS(accessMask?: number): msg.Nfsv4AccessRequest;
    /**
     * READ - Read file data.
     * @param offset - Byte offset to read from
     * @param count - Number of bytes to read
     * @param stateid - State ID (defaults to all zeros)
     */
    READ(offset: bigint, count: number, stateid?: structs.Nfsv4Stateid): msg.Nfsv4ReadRequest;
    /**
     * WRITE - Write file data.
     * @param stateid - State ID to write to
     * @param offset - Byte offset
     * @param stable - Stable flag (Nfsv4StableHow)
     * @param data - Data to write
     */
    WRITE(stateid: structs.Nfsv4Stateid, offset: bigint, stable: number, data: Uint8Array): msg.Nfsv4WriteRequest;
    /**
     * COMMIT - Commit written data to stable storage.
     * @param offset - Byte offset
     * @param count - Number of bytes
     */
    COMMIT(offset: bigint, count: number): msg.Nfsv4CommitRequest;
    /**
     * CREATE - Create a new file.
     * @param objtype - Object type to create
     * @param objname - Name of object to create
     * @param createattrs - Attributes for the new object
     */
    CREATE(objtype: structs.Nfsv4CreateType, objname: string, createattrs: structs.Nfsv4Fattr): msg.Nfsv4CreateRequest;
    /**
     * LINK - Create a hard link.
     * @param newname - Name for the new link
     */
    LINK(newname: string): msg.Nfsv4LinkRequest;
    /**
     * READLINK - Read symbolic link.
     */
    READLINK(): msg.Nfsv4ReadlinkRequest;
    /**
     * SAVEFH - Save current filehandle.
     */
    SAVEFH(): msg.Nfsv4SavefhRequest;
    /**
     * RESTOREFH - Restore saved filehandle to current.
     */
    RESTOREFH(): msg.Nfsv4RestorefhRequest;
    /**
     * SETATTR - Set file attributes.
     * @param stateid - State ID
     * @param attrs - Attributes to set
     */
    SETATTR(stateid: structs.Nfsv4Stateid, attrs: structs.Nfsv4Fattr): msg.Nfsv4SetattrRequest;
    /**
     * VERIFY - Verify attributes match.
     * @param attrs - Attributes to verify
     */
    VERIFY(attrs: structs.Nfsv4Fattr): msg.Nfsv4VerifyRequest;
    /**
     * NVERIFY - Verify attributes don't match.
     * @param attrs - Attributes to verify don't match
     */
    NVERIFY(attrs: structs.Nfsv4Fattr): msg.Nfsv4NverifyRequest;
    /**
     * REMOVE - Remove file or directory.
     * @param name - Name of file/directory to remove
     */
    REMOVE(name: string): msg.Nfsv4RemoveRequest;
    /**
     * RENAME - Rename file or directory.
     * @param oldname - Current name
     * @param newname - New name
     */
    RENAME(oldname: string, newname: string): msg.Nfsv4RenameRequest;
    /**
     * RENEW - Renew client lease.
     * @param clientid - Client ID
     */
    RENEW(clientid: bigint): msg.Nfsv4RenewRequest;
    /**
     * SETCLIENTID - Establish client ID.
     * @param client - Client identifier
     * @param callback - Callback info
     * @param callbackIdent - Callback identifier
     */
    SETCLIENTID(client: structs.Nfsv4ClientId, callback: structs.Nfsv4CbClient, callbackIdent: number): msg.Nfsv4SetclientidRequest;
    /**
     * SETCLIENTID_CONFIRM - Confirm client ID.
     * @param clientid - Client ID to confirm
     * @param verifier - Verifier from SETCLIENTID response
     */
    SETCLIENTID_CONFIRM(clientid: bigint, verifier: structs.Nfsv4Verifier): msg.Nfsv4SetclientidConfirmRequest;
    /**
     * OPEN - Open a file.
     * @param seqid - Sequence ID for open-owner
     * @param shareAccess - Share access mode (OPEN4_SHARE_ACCESS_*)
     * @param shareDeny - Share deny mode (OPEN4_SHARE_DENY_*)
     * @param owner - Open owner (clientid + owner bytes)
     * @param openhow - Open how structure (use OpenHow helper)
     * @param claim - Open claim (use OpenClaim helper)
     */
    OPEN(seqid: number, shareAccess: number, shareDeny: number, owner: structs.Nfsv4OpenOwner, openhow: structs.Nfsv4OpenHow, claim: structs.Nfsv4OpenClaim): msg.Nfsv4OpenRequest;
    /**
     * CLOSE - Close an open file.
     * @param seqid - Sequence ID
     * @param openStateid - State ID from OPEN
     */
    CLOSE(seqid: number, openStateid: structs.Nfsv4Stateid): msg.Nfsv4CloseRequest;
    /**
     * OPEN_CONFIRM - Confirm an open.
     * @param openStateid - State ID from OPEN
     * @param seqid - Sequence ID
     */
    OPEN_CONFIRM(openStateid: structs.Nfsv4Stateid, seqid: number): msg.Nfsv4OpenConfirmRequest;
    /**
     * OPEN_DOWNGRADE - Downgrade open access/deny modes.
     * @param openStateid - State ID from OPEN
     * @param seqid - Sequence ID
     * @param shareAccess - New share access mode
     * @param shareDeny - New share deny mode
     */
    OPEN_DOWNGRADE(openStateid: structs.Nfsv4Stateid, seqid: number, shareAccess: number, shareDeny: number): msg.Nfsv4OpenDowngradeRequest;
    /**
     * OPENATTR - Open named attribute directory.
     * @param createdir - Whether to create the directory if it doesn't exist
     */
    OPENATTR(createdir?: boolean): msg.Nfsv4OpenattrRequest;
    /**
     * SECINFO - Get security information for a file.
     * @param name - Filename to get security info for
     */
    SECINFO(name: string): msg.Nfsv4SecinfoRequest;
    /**
     * DELEGPURGE - Purge delegations (not supported).
     * @param clientid - Client ID
     */
    DELEGPURGE(clientid: bigint): msg.Nfsv4DelegpurgeRequest;
    /**
     * DELEGRETURN - Return delegation (not supported).
     * @param stateid - Delegation stateid
     */
    DELEGRETURN(stateid: structs.Nfsv4Stateid): msg.Nfsv4DelegreturnRequest;
    /**
     * LOCK - Lock byte range.
     * @param locktype - Lock type (READ_LT, WRITE_LT, READW_LT, or WRITEW_LT)
     * @param reclaim - True if reclaiming lock after server restart
     * @param offset - Starting byte offset
     * @param length - Length in bytes (0xFFFFFFFFFFFFFFFF for EOF)
     * @param locker - Lock owner info (new or existing lock owner)
     */
    LOCK(locktype: Nfsv4LockType, reclaim: boolean, offset: bigint, length: bigint, locker: structs.Nfsv4LockOwnerInfo): msg.Nfsv4LockRequest;
    /**
     * LOCKT - Test for conflicting lock (non-blocking).
     * @param locktype - Lock type (READ_LT or WRITE_LT)
     * @param offset - Starting byte offset
     * @param length - Length in bytes (0xFFFFFFFFFFFFFFFF for EOF)
     * @param owner - Lock owner
     */
    LOCKT(locktype: number, offset: bigint, length: bigint, owner: structs.Nfsv4LockOwner): msg.Nfsv4LocktRequest;
    /**
     * LOCKU - Unlock byte range.
     * @param locktype - Lock type (READ_LT or WRITE_LT)
     * @param seqid - Sequence number
     * @param lockStateid - Lock stateid from LOCK operation
     * @param offset - Starting byte offset
     * @param length - Length in bytes
     */
    LOCKU(locktype: number, seqid: number, lockStateid: structs.Nfsv4Stateid, offset: bigint, length: bigint): msg.Nfsv4LockuRequest;
    /**
     * RELEASE_LOCKOWNER - Release all locks for a lock-owner.
     * @param lockOwner - Lock owner to release
     */
    RELEASE_LOCKOWNER(lockOwner: structs.Nfsv4LockOwner): msg.Nfsv4ReleaseLockOwnerRequest;
    /**
     * Create an Nfsv4Verifier (8-byte opaque data).
     * @param data - 8-byte Uint8Array, defaults to zeros
     */
    Verifier(data?: Uint8Array): structs.Nfsv4Verifier;
    /**
     * Create an Nfsv4Stateid (state identifier).
     * @param seqid - Sequence ID (default: 0)
     * @param other - 12-byte opaque data (default: zeros)
     */
    Stateid(seqid?: number, other?: Uint8Array): structs.Nfsv4Stateid;
    /**
     * Create Nfsv4Fattr from attribute numbers (automatically converts to bitmap).
     * @param attrNums - Array of attribute numbers (Nfsv4Attr enum values)
     * @param attrVals - Encoded attribute values as byte array
     */
    Fattr(attrNums: number[], attrVals: Uint8Array): structs.Nfsv4Fattr;
    /**
     * Create Nfsv4ClientId (client identifier).
     * @param verifier - 8-byte verifier
     * @param id - Variable-length client ID bytes
     */
    ClientId(verifier: structs.Nfsv4Verifier, id: Uint8Array): structs.Nfsv4ClientId;
    /**
     * Create Nfsv4CbClient (callback client information).
     * @param cbProgram - Callback program number
     * @param rNetid - Network ID string (e.g., 'tcp', 'udp')
     * @param rAddr - Network address string (e.g., '127.0.0.1.8.1')
     */
    CbClient(cbProgram: number, rNetid: string, rAddr: string): structs.Nfsv4CbClient;
    /**
     * Create Nfsv4Bitmap from attribute numbers.
     * @param attrNums - Array of attribute numbers (Nfsv4Attr enum values)
     */
    Bitmap(attrNums: number[]): structs.Nfsv4Bitmap;
    /**
     * Create Nfsv4CreateType for regular file creation.
     */
    CreateTypeFile(): structs.Nfsv4CreateType;
    /**
     * Create Nfsv4CreateType for directory creation.
     */
    CreateTypeDir(): structs.Nfsv4CreateType;
    /**
     * Create Nfsv4OpenOwner (open owner identifier).
     * @param clientid - Client ID
     * @param owner - Owner bytes (unique identifier)
     */
    OpenOwner(clientid: bigint, owner: Uint8Array): structs.Nfsv4OpenOwner;
    /**
     * Create Nfsv4OpenClaim for CLAIM_NULL (open by filename).
     * @param filename - Name of file to open
     */
    OpenClaimNull(filename: string): structs.Nfsv4OpenClaim;
    /**
     * Create Nfsv4OpenHow for OPEN4_NOCREATE (open existing file).
     */
    OpenHowNoCreate(): structs.Nfsv4OpenHow;
    /**
     * Create Nfsv4OpenHow for OPEN4_CREATE with UNCHECKED4 mode.
     * @param createattrs - Optional file attributes to set on create
     */
    OpenHowCreateUnchecked(createattrs?: structs.Nfsv4Fattr): structs.Nfsv4OpenHow;
    /**
     * Create Nfsv4OpenHow for OPEN4_CREATE with GUARDED4 mode.
     * @param createattrs - Optional file attributes to set on create
     */
    OpenHowCreateGuarded(createattrs?: structs.Nfsv4Fattr): structs.Nfsv4OpenHow;
    /**
     * Create Nfsv4OpenHow for OPEN4_CREATE with EXCLUSIVE4 mode.
     * @param verifier - 8-byte verifier for exclusive create
     */
    OpenHowCreateExclusive(verifier: structs.Nfsv4Verifier): structs.Nfsv4OpenHow;
    /**
     * Create Nfsv4LockOwner (lock owner identifier).
     * @param clientid - Client ID
     * @param owner - Owner bytes (unique identifier)
     */
    LockOwner(clientid: bigint, owner: Uint8Array): structs.Nfsv4LockOwner;
    /**
     * Create Nfsv4LockOwnerInfo for new lock owner (open_to_lock_owner).
     * @param openSeqid - Current open-owner seqid
     * @param openStateid - Open stateid from OPEN operation
     * @param lockSeqid - Initial lock-owner seqid (typically 0)
     * @param lockOwner - Lock owner identifier
     */
    NewLockOwner(openSeqid: number, openStateid: structs.Nfsv4Stateid, lockSeqid: number, lockOwner: structs.Nfsv4LockOwner): structs.Nfsv4LockOwnerInfo;
    /**
     * Create Nfsv4LockOwnerInfo for existing lock owner.
     * @param lockStateid - Lock stateid from previous LOCK operation
     * @param lockSeqid - Lock-owner seqid
     */
    ExistingLockOwner(lockStateid: structs.Nfsv4Stateid, lockSeqid: number): structs.Nfsv4LockOwnerInfo;
    /**
     * ILLEGAL - Illegal operation (for testing RFC 7530 §15.2.4 compliance).
     * This operation is used to test server handling of illegal operation codes.
     * Per RFC 7530, the server should respond with NFS4ERR_OP_ILLEGAL.
     */
    ILLEGAL(): msg.Nfsv4IllegalRequest;
};
