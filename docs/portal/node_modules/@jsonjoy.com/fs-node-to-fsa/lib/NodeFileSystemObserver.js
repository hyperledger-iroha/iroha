"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.NodeFileSystemObserver = void 0;
const fs_fsa_1 = require("@jsonjoy.com/fs-fsa");
const NodeFileSystemDirectoryHandle_1 = require("./NodeFileSystemDirectoryHandle");
const NodeFileSystemFileHandle_1 = require("./NodeFileSystemFileHandle");
const util_1 = require("./util");
/**
 * A `FileSystemObserver` implementation backed by the underlying Node.js-like
 * `fs.watch`. This is a best-effort profile, per the File System Observer
 * proposal's allowance for local file systems: `rename` events are classified
 * into `"appeared"`/`"disappeared"` records by stat-ing the path, no
 * `"moved"` records are ever produced (pairing renames is unreliable — same
 * as Chrome on Windows), and a backend watcher error surfaces as a terminal
 * `"errored"` record for that observation.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemObserver
 */
class NodeFileSystemObserver {
    constructor(fs, callback) {
        this.fs = fs;
        this.callback = callback;
        this._observations = new Map();
        this._records = [];
        this._flushScheduled = false;
    }
    async observe(handle, options) {
        const path = handle.__path;
        const ctx = (handle.ctx ?? handle._ctx);
        if (typeof path !== 'string' || !ctx || (ctx.separator !== '/' && ctx.separator !== '\\'))
            throw new TypeError("Failed to execute 'observe' on 'FileSystemObserver': Invalid handle.");
        const isDirectory = handle.kind === 'directory';
        const last = path[path.length - 1];
        const isTrimmableSeparator = path.length > 1 && (last === '/' || last === '\\') && path[path.length - 2] !== ':';
        const target = isTrimmableSeparator ? path.slice(0, -1) : path;
        try {
            await this.fs.promises.stat(target);
        }
        catch (error) {
            if (error && typeof error === 'object') {
                switch (error.code) {
                    case 'ENOENT':
                        throw (0, util_1.newNotFoundError)();
                    case 'EACCES':
                    case 'EPERM':
                        throw (0, util_1.newNotAllowedError)();
                }
            }
            throw error;
        }
        const recursive = isDirectory && !!options?.recursive;
        const watcher = this.fs.watch(target, { recursive }, (eventType, filename) => {
            void this.onEvent(handle, watcher, target, isDirectory, ctx, eventType, filename ? String(filename) : '');
        });
        watcher.on('error', () => {
            if (this._observations.get(handle) !== watcher)
                return;
            this._observations.delete(handle);
            watcher.close();
            this._enqueue(new fs_fsa_1.FileSystemChangeRecord(handle, 'errored', null, []));
        });
        this._observations.get(handle)?.close();
        this._observations.set(handle, watcher);
    }
    unobserve(handle) {
        const watcher = this._observations.get(handle);
        if (!watcher)
            return;
        watcher.close();
        this._observations.delete(handle);
    }
    /** Disconnect and stop all observations. */
    disconnect() {
        for (const watcher of this._observations.values())
            watcher.close();
        this._observations.clear();
        this._records = [];
    }
    async onEvent(root, watcher, rootPath, isDirectory, ctx, eventType, filename) {
        const sep = ctx.separator;
        const steps = isDirectory && filename ? filename.split(sep) : [];
        const absolute = isDirectory ? (rootPath === sep ? rootPath + filename : rootPath + sep + filename) : rootPath;
        let stats = null;
        try {
            stats = (await this.fs.promises.stat(absolute));
        }
        catch {
            stats = null;
        }
        if (this._observations.get(root) !== watcher)
            return;
        if (eventType === 'rename') {
            if (stats) {
                this._enqueue(new fs_fsa_1.FileSystemChangeRecord(root, 'appeared', this._handle(absolute, stats, ctx), steps));
            }
            else {
                this._enqueue(new fs_fsa_1.FileSystemChangeRecord(root, 'disappeared', null, steps));
            }
        }
        else if (stats) {
            this._enqueue(new fs_fsa_1.FileSystemChangeRecord(root, 'modified', this._handle(absolute, stats, ctx), steps));
        }
    }
    _handle(absolute, stats, ctx) {
        return stats.isDirectory()
            ? new NodeFileSystemDirectoryHandle_1.NodeFileSystemDirectoryHandle(this.fs, absolute, ctx)
            : new NodeFileSystemFileHandle_1.NodeFileSystemFileHandle(this.fs, absolute, ctx);
    }
    _enqueue(record) {
        this._records.push(record);
        if (this._flushScheduled)
            return;
        this._flushScheduled = true;
        queueMicrotask(() => {
            this._flushScheduled = false;
            const records = this._records;
            this._records = [];
            if (records.length)
                this.callback(records, this);
        });
    }
}
exports.NodeFileSystemObserver = NodeFileSystemObserver;
//# sourceMappingURL=NodeFileSystemObserver.js.map