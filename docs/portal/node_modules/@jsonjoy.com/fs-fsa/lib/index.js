"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.fsa = void 0;
const tslib_1 = require("tslib");
const CoreFileSystemDirectoryHandle_1 = require("./CoreFileSystemDirectoryHandle");
const fs_core_1 = require("@jsonjoy.com/fs-core");
const CoreFileSystemObserver_1 = require("./CoreFileSystemObserver");
tslib_1.__exportStar(require("./types"), exports);
tslib_1.__exportStar(require("./CoreFileSystemHandle"), exports);
tslib_1.__exportStar(require("./CoreFileSystemDirectoryHandle"), exports);
tslib_1.__exportStar(require("./CoreFileSystemFileHandle"), exports);
tslib_1.__exportStar(require("./CoreFileSystemSyncAccessHandle"), exports);
tslib_1.__exportStar(require("./CoreFileSystemWritableFileStream"), exports);
tslib_1.__exportStar(require("./CoreFileSystemObserver"), exports);
tslib_1.__exportStar(require("./CorePermissionStatus"), exports);
tslib_1.__exportStar(require("./FileLockManager"), exports);
/**
 * Create a new instance of an in-memory File System Access API
 * implementation rooted at the root directory of the filesystem.
 *
 * @param ctx Optional context for the File System Access API.
 * @param core Optional low-level file system implementation to
 *     back the File System Access API. If not provided, a new empty
 *     Superblock instance will be created.
 * @param dirPath Optional path within the filesystem to use as the root
 *     directory of the File System Access API. Defaults to `/`.
 * @returns A File System Access API implementation `dir` rooted at
 *     the root directory of the filesystem, as well as the `core`,
 *     a low-level file system implementation itself. Also, returns
 *     `FileSystemObserver`, a class that can be used to create
 *     observers that watch for changes to files and directories.
 */
const fsa = (ctx, core = new fs_core_1.Superblock(), dirPath = '/') => {
    const dir = new CoreFileSystemDirectoryHandle_1.CoreFileSystemDirectoryHandle(core, dirPath, ctx);
    const FileSystemObserver = class FileSystemObserver extends CoreFileSystemObserver_1.CoreFileSystemObserver {
        constructor(callback) {
            super(core, callback);
        }
    };
    return { core, dir, FileSystemObserver };
};
exports.fsa = fsa;
//# sourceMappingURL=index.js.map