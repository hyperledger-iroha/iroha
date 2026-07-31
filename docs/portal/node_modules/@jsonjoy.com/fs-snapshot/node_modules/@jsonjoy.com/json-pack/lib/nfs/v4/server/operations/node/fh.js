"use strict";
/**
 * @module File handle (FH) operations for NFS v4 server.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.FileHandleMapper = exports.decodePathFh = exports.encodePathFh = exports.ROOT_FH = void 0;
const encode_1 = require("@jsonjoy.com/buffers/lib/utf8/encode");
const decodeUtf8_1 = require("@jsonjoy.com/buffers/lib/utf8/decodeUtf8");
const node_crypto_1 = require("node:crypto");
exports.ROOT_FH = new Uint8Array([0]);
/**
 * Encodes a file path as a Type 1 file handle (path-based).
 * Format: `[FH_TYPE.PATH, ...utf8PathBytes]`
 *
 * @returns The encoded file handle, or undefined if the path is too long.
 */
const encodePathFh = (absolutePath) => {
    const utf8Length = Buffer.byteLength(absolutePath, 'utf8');
    if (utf8Length + 1 > 128 /* FH.MAX_SIZE */)
        return undefined;
    const u8 = new Uint8Array(1 + utf8Length);
    u8[0] = 1 /* FH_TYPE.PATH */;
    (0, encode_1.encode)(u8, absolutePath, 1, utf8Length);
    return u8;
};
exports.encodePathFh = encodePathFh;
const decodePathFh = (fh) => {
    const length = fh.length;
    if (length < 2)
        return undefined;
    if (fh[0] !== 1 /* FH_TYPE.PATH */)
        return undefined;
    return (0, decodeUtf8_1.decodeUtf8)(fh, 1, length - 1);
};
exports.decodePathFh = decodePathFh;
class FileHandleMapper {
    constructor(stamp, 
    /** Root directory for all file handles. */
    dir) {
        this.dir = dir;
        /** Map from random ID (40 bits) to absolute file path for Type 2 file handles. */
        this.idToPath = new Map();
        this.pathToId = new Map();
        this.maxFhTableSize = 100000;
        this.stamp = stamp & 0xffff;
    }
    /**
     * Decodes a file handle to an absolute file path.
     * Returns `undefined` if the file handle could not be decoded.
     */
    decode(fh) {
        const length = fh.length;
        if (fh.length === 0)
            return this.dir;
        const type = fh[0];
        if (type === 0 /* FH_TYPE.ROOT */)
            return this.dir;
        if (type === 1 /* FH_TYPE.PATH */) {
            try {
                const path = (0, exports.decodePathFh)(fh);
                if (!path)
                    throw 10001 /* Nfsv4Stat.NFS4ERR_BADHANDLE */;
                return path;
            }
            catch {
                throw 10001 /* Nfsv4Stat.NFS4ERR_BADHANDLE */;
            }
        }
        if (type === 2 /* FH_TYPE.ID */) {
            if (length !== 8)
                throw 10001 /* Nfsv4Stat.NFS4ERR_BADHANDLE */;
            const stamp = (fh[1] << 8) | fh[2];
            if (stamp !== this.stamp)
                throw 10014 /* Nfsv4Stat.NFS4ERR_FHEXPIRED */;
            const id = fh[3] * 0x100000000 + fh[4] * 0x1000000 + (fh[5] << 16) + (fh[6] << 8) + fh[7];
            const path = this.idToPath.get(id);
            if (!path)
                throw 10014 /* Nfsv4Stat.NFS4ERR_FHEXPIRED */;
            return path;
        }
        throw 10001 /* Nfsv4Stat.NFS4ERR_BADHANDLE */;
    }
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
    encode(path) {
        if (path === this.dir)
            return exports.ROOT_FH;
        // let fh = encodePathFh(path);
        // if (fh) return fh;
        let fh = this.pathToId.get(path);
        if (fh)
            return fh;
        fh = (0, node_crypto_1.randomBytes)(8);
        fh[0] = 2 /* FH_TYPE.ID */;
        fh[1] = (this.stamp >> 8) & 0xff;
        fh[2] = this.stamp & 0xff;
        const id = fh[3] * 0x100000000 + fh[4] * 0x1000000 + (fh[5] << 16) + (fh[6] << 8) + fh[7];
        const { idToPath, pathToId, maxFhTableSize } = this;
        ENFORCE_FH_TABLE_SIZE_LIMIT: {
            if (idToPath.size <= maxFhTableSize)
                break ENFORCE_FH_TABLE_SIZE_LIMIT;
            const entry = idToPath.entries().next().value;
            if (entry) {
                const [id, path] = entry;
                idToPath.delete(id);
                pathToId.delete(path);
            }
        }
        idToPath.set(id, path);
        pathToId.set(path, fh);
        return fh;
    }
    /**
     * Perform a basic quick validation of the file handle structure.
     * This does not guarantee that the file handle is valid, only that
     * it is likely to be well-formed.
     */
    validate(fh) {
        if (fh.length === 0)
            return true;
        const type = fh[0];
        if (type === 0 /* FH_TYPE.ROOT */)
            return true;
        if (type === 1 /* FH_TYPE.PATH */)
            return true;
        if (type === 2 /* FH_TYPE.ID */)
            return true;
        return false;
    }
    /**
     * Gets the current file path from the operation context.
     * @param ctx Operation context containing the current file handle (cfh).
     * @returns The current file path.
     */
    currentPath(ctx) {
        const cfh = ctx.cfh;
        if (!cfh)
            throw 10020 /* Nfsv4Stat.NFS4ERR_NOFILEHANDLE */;
        return this.decode(cfh);
    }
    /**
     * Gets the saved file path from the operation context.
     * @param ctx Operation context containing the saved file handle (sfh).
     * @returns The saved file path.
     */
    savedPath(ctx) {
        const sfh = ctx.sfh;
        if (!sfh)
            throw 10020 /* Nfsv4Stat.NFS4ERR_NOFILEHANDLE */;
        return this.decode(sfh);
    }
    /**
     * Sets the current file handle in the operation context to the given path.
     * @param ctx Operation context to update.
     * @param path Absolute file path to set as the current file handle.
     */
    setCfh(ctx, path) {
        const newFh = this.encode(path);
        ctx.cfh = newFh;
    }
    /**
     * Removes a file handle mapping for the given path.
     * This is used when a file is deleted or replaced.
     * @param path The absolute file path to remove from the mapping.
     */
    remove(path) {
        const fh = this.pathToId.get(path);
        if (!fh)
            return;
        const type = fh[0];
        if (type !== 2 /* FH_TYPE.ID */)
            return;
        const id = fh[3] * 0x100000000 + fh[4] * 0x1000000 + (fh[5] << 16) + (fh[6] << 8) + fh[7];
        this.pathToId.delete(path);
        this.idToPath.delete(id);
    }
    /**
     * Updates the file handle mappings when a file is renamed.
     * This ensures that existing file handles pointing to the old path
     * continue to work after the rename operation.
     * When renaming over an existing file, the destination file handle
     * is removed from the cache since that file will be replaced.
     * @param oldPath The old absolute file path.
     * @param newPath The new absolute file path.
     */
    rename(oldPath, newPath) {
        this.remove(newPath);
        const fh = this.pathToId.get(oldPath);
        if (!fh)
            return;
        const type = fh[0];
        if (type !== 2 /* FH_TYPE.ID */)
            return;
        const id = fh[3] * 0x100000000 + fh[4] * 0x1000000 + (fh[5] << 16) + (fh[6] << 8) + fh[7];
        this.pathToId.delete(oldPath);
        this.pathToId.set(newPath, fh);
        this.idToPath.set(id, newPath);
    }
}
exports.FileHandleMapper = FileHandleMapper;
//# sourceMappingURL=fh.js.map