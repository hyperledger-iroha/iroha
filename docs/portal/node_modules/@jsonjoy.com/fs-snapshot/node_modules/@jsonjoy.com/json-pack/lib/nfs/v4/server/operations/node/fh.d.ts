/**
 * @module File handle (FH) operations for NFS v4 server.
 */
import type { Nfsv4OperationCtx } from '../Nfsv4Operations';
export declare const ROOT_FH: Uint8Array<ArrayBuffer>;
export declare const enum FH_TYPE {
    /** Root file handle. */
    ROOT = 0,
    /** Path file handle: the full path is encoded in the file handle. */
    PATH = 1,
    /** ID file handle: server stores the mapping between the ID and the file path. */
    ID = 2
}
export declare const enum FH {
    MAX_SIZE = 128
}
/**
 * Encodes a file path as a Type 1 file handle (path-based).
 * Format: `[FH_TYPE.PATH, ...utf8PathBytes]`
 *
 * @returns The encoded file handle, or undefined if the path is too long.
 */
export declare const encodePathFh: (absolutePath: string) => Uint8Array | undefined;
export declare const decodePathFh: (fh: Uint8Array) => string | undefined;
export declare class FileHandleMapper {
    /** Root directory for all file handles. */
    protected readonly dir: string;
    /** 16-bit unsigned int which identifies this server instance. */
    protected readonly stamp: number;
    /** Map from random ID (40 bits) to absolute file path for Type 2 file handles. */
    protected idToPath: Map<number, string>;
    protected pathToId: Map<string, Uint8Array>;
    protected readonly maxFhTableSize = 100000;
    constructor(stamp: number, 
    /** Root directory for all file handles. */
    dir: string);
    /**
     * Decodes a file handle to an absolute file path.
     * Returns `undefined` if the file handle could not be decoded.
     */
    decode(fh: Uint8Array): string;
    /**
     * Encodes a file path as a file handle. Uses Type 1 (path-based) if the path
     * fits, otherwise uses Type 2 (ID-based).
     *
     * Type-2 Format:
     *
     * - 1 byte: FH_TYPE.ID
     * - 2 bytes: boot stamp (server instance ID)
     * - 5 bytes: random ID (unique per file handle)
     */
    encode(path: string): Uint8Array;
    /**
     * Perform a basic quick validation of the file handle structure.
     * This does not guarantee that the file handle is valid, only that
     * it is likely to be well-formed.
     */
    validate(fh: Uint8Array): boolean;
    /**
     * Gets the current file path from the operation context.
     * @param ctx Operation context containing the current file handle (cfh).
     * @returns The current file path.
     */
    currentPath(ctx: Nfsv4OperationCtx): string;
    /**
     * Gets the saved file path from the operation context.
     * @param ctx Operation context containing the saved file handle (sfh).
     * @returns The saved file path.
     */
    savedPath(ctx: Nfsv4OperationCtx): string;
    /**
     * Sets the current file handle in the operation context to the given path.
     * @param ctx Operation context to update.
     * @param path Absolute file path to set as the current file handle.
     */
    setCfh(ctx: Nfsv4OperationCtx, path: string): void;
    /**
     * Removes a file handle mapping for the given path.
     * This is used when a file is deleted or replaced.
     * @param path The absolute file path to remove from the mapping.
     */
    remove(path: string): void;
    /**
     * Updates the file handle mappings when a file is renamed.
     * This ensures that existing file handles pointing to the old path
     * continue to work after the rename operation.
     * When renaming over an existing file, the destination file handle
     * is removed from the cache since that file will be replaced.
     * @param oldPath The old absolute file path.
     * @param newPath The new absolute file path.
     */
    rename(oldPath: string, newPath: string): void;
}
