"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.FilesystemStats = void 0;
/**
 * Filesystem statistics for NFSv4 space and file count attributes.
 */
class FilesystemStats {
    constructor(
    /** Available space in bytes for unprivileged users */
    spaceAvail, 
    /** Free space in bytes on the filesystem */
    spaceFree, 
    /** Total space in bytes on the filesystem */
    spaceTotal, 
    /** Available file slots (inodes) */
    filesAvail, 
    /** Free file slots (inodes) */
    filesFree, 
    /** Total file slots (inodes) */
    filesTotal) {
        this.spaceAvail = spaceAvail;
        this.spaceFree = spaceFree;
        this.spaceTotal = spaceTotal;
        this.filesAvail = filesAvail;
        this.filesFree = filesFree;
        this.filesTotal = filesTotal;
    }
}
exports.FilesystemStats = FilesystemStats;
//# sourceMappingURL=FilesystemStats.js.map