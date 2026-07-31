"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.NfsFsDirent = void 0;
/**
 * Implements Node.js-like Dirent interface for NFS v4 directory entries.
 */
class NfsFsDirent {
    constructor(name, type) {
        this.name = name;
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
exports.NfsFsDirent = NfsFsDirent;
//# sourceMappingURL=NfsFsDirent.js.map