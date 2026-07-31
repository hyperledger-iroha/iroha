"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.NfsFsStats = void 0;
/**
 * Implements Node.js-like Stats interface for NFS v4 file attributes.
 */
class NfsFsStats {
    constructor(uid, gid, rdev, blksize, ino, size, blocks, atime, mtime, ctime, birthtime, atimeMs, mtimeMs, ctimeMs, birthtimeMs, dev, mode, nlink, type) {
        this.uid = uid;
        this.gid = gid;
        this.rdev = rdev;
        this.blksize = blksize;
        this.ino = ino;
        this.size = size;
        this.blocks = blocks;
        this.atime = atime;
        this.mtime = mtime;
        this.ctime = ctime;
        this.birthtime = birthtime;
        this.atimeMs = atimeMs;
        this.mtimeMs = mtimeMs;
        this.ctimeMs = ctimeMs;
        this.birthtimeMs = birthtimeMs;
        this.dev = dev;
        this.mode = mode;
        this.nlink = nlink;
        this.type = type;
    }
    isDirectory() {
        return this.type === 2 /* Nfsv4FType.NF4DIR */;
    }
    isFile() {
        return this.type === 1 /* Nfsv4FType.NF4REG */;
    }
    isBlockDevice() {
        return this.type === 3 /* Nfsv4FType.NF4BLK */;
    }
    isCharacterDevice() {
        return this.type === 4 /* Nfsv4FType.NF4CHR */;
    }
    isSymbolicLink() {
        return this.type === 5 /* Nfsv4FType.NF4LNK */;
    }
    isFIFO() {
        return this.type === 7 /* Nfsv4FType.NF4FIFO */;
    }
    isSocket() {
        return this.type === 6 /* Nfsv4FType.NF4SOCK */;
    }
}
exports.NfsFsStats = NfsFsStats;
//# sourceMappingURL=NfsFsStats.js.map