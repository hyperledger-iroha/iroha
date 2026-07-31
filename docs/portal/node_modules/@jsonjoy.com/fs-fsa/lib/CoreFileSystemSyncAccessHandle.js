"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CoreFileSystemSyncAccessHandle = void 0;
const buffer_1 = require("@jsonjoy.com/fs-node-builtins/lib/internal/buffer");
const util_1 = require("./util");
const fs_node_utils_1 = require("@jsonjoy.com/fs-node-utils");
/**
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle
 */
class CoreFileSystemSyncAccessHandle {
    constructor(_core, __path, _ctx) {
        this._core = _core;
        this.__path = __path;
        this._ctx = _ctx;
        this._fd = null;
        this._closed = false;
        this._ctx.locks.acquireLock(this.__path);
    }
    _ensureOpen() {
        if (this._closed) {
            throw new DOMException('The file handle is closed.', 'InvalidStateError');
        }
        if (this._fd === null) {
            // Open file for read/write
            const flags = this._ctx.mode === 'readwrite' ? fs_node_utils_1.FLAGS['r+'] : fs_node_utils_1.FLAGS.r;
            this._fd = this._core.open(this.__path, flags, 0o644);
        }
        return this._fd;
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/close
     */
    async close() {
        if (this._fd !== null) {
            this._core.close(this._fd);
            this._fd = null;
        }
        this._closed = true;
        this._ctx.locks.releaseLock(this.__path);
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/flush
     */
    async flush() {
        const fd = this._ensureOpen();
        // Core doesn't have an explicit flush method, but we can try to sync if available
        // For now, this is a no-op as the core writes are synchronous
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/getSize
     */
    async getSize() {
        try {
            const link = this._core.getResolvedLinkOrThrow(this.__path);
            const node = link.getNode();
            return node.getSize();
        }
        catch (error) {
            if (error && typeof error === 'object' && error.code === "EACCES" /* ERROR_CODE.EACCES */) {
                throw (0, util_1.newNotAllowedError)();
            }
            throw error;
        }
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/read
     */
    async read(buffer, options = {}) {
        const fd = this._ensureOpen();
        const { at: position = 0 } = options;
        const buf = buffer_1.Buffer.from(buffer);
        try {
            return this._core.read(fd, buf, 0, buf.length, position);
        }
        catch (error) {
            if (error && typeof error === 'object' && error.code === "EACCES" /* ERROR_CODE.EACCES */) {
                throw (0, util_1.newNotAllowedError)();
            }
            throw error;
        }
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/truncate
     */
    async truncate(newSize) {
        if (this._ctx.mode !== 'readwrite') {
            throw (0, util_1.newNotAllowedError)();
        }
        try {
            const fd = this._ensureOpen();
            this._core.ftruncate(fd, newSize);
        }
        catch (error) {
            if (error && typeof error === 'object' && error.code === "EACCES" /* ERROR_CODE.EACCES */) {
                throw (0, util_1.newNotAllowedError)();
            }
            throw error;
        }
    }
    /**
     * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle/write
     */
    async write(buffer, options = {}) {
        if (this._ctx.mode !== 'readwrite') {
            throw (0, util_1.newNotAllowedError)();
        }
        const fd = this._ensureOpen();
        const { at: position = 0 } = options;
        const buf = buffer_1.Buffer.from(buffer);
        try {
            return this._core.write(fd, buf, 0, buf.length, position);
        }
        catch (error) {
            if (error && typeof error === 'object' && error.code === "EACCES" /* ERROR_CODE.EACCES */) {
                throw (0, util_1.newNotAllowedError)();
            }
            throw error;
        }
    }
}
exports.CoreFileSystemSyncAccessHandle = CoreFileSystemSyncAccessHandle;
//# sourceMappingURL=CoreFileSystemSyncAccessHandle.js.map