/**
 * Filesystem statistics for NFSv4 space and file count attributes.
 */
export declare class FilesystemStats {
    /** Available space in bytes for unprivileged users */
    readonly spaceAvail: bigint;
    /** Free space in bytes on the filesystem */
    readonly spaceFree: bigint;
    /** Total space in bytes on the filesystem */
    readonly spaceTotal: bigint;
    /** Available file slots (inodes) */
    readonly filesAvail: bigint;
    /** Free file slots (inodes) */
    readonly filesFree: bigint;
    /** Total file slots (inodes) */
    readonly filesTotal: bigint;
    constructor(
    /** Available space in bytes for unprivileged users */
    spaceAvail: bigint, 
    /** Free space in bytes on the filesystem */
    spaceFree: bigint, 
    /** Total space in bytes on the filesystem */
    spaceTotal: bigint, 
    /** Available file slots (inodes) */
    filesAvail: bigint, 
    /** Free file slots (inodes) */
    filesFree: bigint, 
    /** Total file slots (inodes) */
    filesTotal: bigint);
}
