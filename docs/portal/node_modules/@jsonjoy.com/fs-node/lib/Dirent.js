"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Dirent = void 0;
const fs_node_utils_1 = require("@jsonjoy.com/fs-node-utils");
const { S_IFMT, S_IFDIR, S_IFREG, S_IFBLK, S_IFCHR, S_IFLNK, S_IFIFO, S_IFSOCK } = fs_node_utils_1.constants;
/**
 * A directory entry, like `fs.Dirent`.
 */
class Dirent {
    constructor() {
        this.name = '';
        this.parentPath = '';
        this.mode = 0;
        /**
         * @deprecated Will be removed at any time.
         * @see https://nodejs.org/api/deprecations.html#DEP0178
         */
        this.path = '';
    }
    static build(link, encoding) {
        const dirent = new Dirent();
        const { mode } = link.getNode();
        dirent.name = (0, fs_node_utils_1.strToEncoding)(link.getName(), encoding);
        dirent.mode = mode;
        dirent.parentPath = link.getParentPath();
        dirent.path = dirent.parentPath;
        return dirent;
    }
    _checkModeProperty(property) {
        return (this.mode & S_IFMT) === property;
    }
    isDirectory() {
        return this._checkModeProperty(S_IFDIR);
    }
    isFile() {
        return this._checkModeProperty(S_IFREG);
    }
    isBlockDevice() {
        return this._checkModeProperty(S_IFBLK);
    }
    isCharacterDevice() {
        return this._checkModeProperty(S_IFCHR);
    }
    isSymbolicLink() {
        return this._checkModeProperty(S_IFLNK);
    }
    isFIFO() {
        return this._checkModeProperty(S_IFIFO);
    }
    isSocket() {
        return this._checkModeProperty(S_IFSOCK);
    }
}
exports.Dirent = Dirent;
exports.default = Dirent;
//# sourceMappingURL=Dirent.js.map