"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CoreFileSystemObserver = exports.FileSystemChangeRecord = void 0;
const CoreFileSystemDirectoryHandle_1 = require("./CoreFileSystemDirectoryHandle");
const CoreFileSystemFileHandle_1 = require("./CoreFileSystemFileHandle");
const util_1 = require("./util");
const fs_core_1 = require("@jsonjoy.com/fs-core");
/**
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemChangeRecord
 */
class FileSystemChangeRecord {
    constructor(root, type, changedHandle, relativePathComponents, relativePathMovedFrom = null) {
        this.root = root;
        this.type = type;
        this.changedHandle = changedHandle;
        this.relativePathComponents = relativePathComponents;
        this.relativePathMovedFrom = relativePathMovedFrom;
    }
}
exports.FileSystemChangeRecord = FileSystemChangeRecord;
/**
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemObserver
 */
class CoreFileSystemObserver {
    constructor(_core, callback) {
        this._core = _core;
        this.callback = callback;
        this._observations = new Map();
        this._records = [];
        this._flushScheduled = false;
    }
    async observe(handle, options) {
        const path = handle.__path;
        if (typeof path !== 'string')
            throw new TypeError("Failed to execute 'observe' on 'FileSystemObserver': Invalid handle.");
        const isDirectory = handle.kind === 'directory';
        const last = path[path.length - 1];
        const target = path.length > 1 && (last === '/' || last === '\\') ? path.slice(0, -1) : path;
        const ctx = (handle.ctx ?? handle._ctx);
        try {
            const watcher = new fs_core_1.CoreWatcher(this._core, target, { recursive: isDirectory && !!options?.recursive });
            watcher.changes.listen(event => this._onEvent(event, handle, watcher, ctx));
            this._observations.get(handle)?.close();
            this._observations.set(handle, watcher);
        }
        catch (error) {
            if (error && typeof error === 'object') {
                switch (error.code) {
                    case "ENOENT" /* ERROR_CODE.ENOENT */:
                        throw (0, util_1.newNotFoundError)();
                    case "EACCES" /* ERROR_CODE.EACCES */:
                        throw (0, util_1.newNotAllowedError)();
                }
            }
            throw error;
        }
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
    _onEvent(event, root, watcher, ctx) {
        let type;
        let changedHandle = null;
        let relativePathMovedFrom = null;
        switch (event.type) {
            case 0 /* FsEventType.CREATE */:
                type = 'appeared';
                changedHandle = this._changedHandle(event, watcher, ctx);
                break;
            case 1 /* FsEventType.DELETE */:
                if (!event.steps.length) {
                    type = 'errored';
                    if (this._observations.get(root) === watcher)
                        this._observations.delete(root);
                }
                else {
                    type = 'disappeared';
                }
                break;
            case 4 /* FsEventType.MOVE */:
                type = 'moved';
                changedHandle = this._changedHandle(event, watcher, ctx);
                relativePathMovedFrom = event.oldSteps ?? null;
                break;
            default:
                type = 'modified';
                changedHandle = this._changedHandle(event, watcher, ctx);
        }
        this._enqueue(new FileSystemChangeRecord(root, type, changedHandle, event.steps, relativePathMovedFrom));
    }
    _changedHandle(event, watcher, ctx) {
        const sep = ctx.separator;
        const path = watcher.link.steps.concat(event.steps).join(sep) || sep;
        return event.node.isDirectory()
            ? new CoreFileSystemDirectoryHandle_1.CoreFileSystemDirectoryHandle(this._core, path, ctx)
            : new CoreFileSystemFileHandle_1.CoreFileSystemFileHandle(this._core, path, ctx);
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
exports.CoreFileSystemObserver = CoreFileSystemObserver;
//# sourceMappingURL=CoreFileSystemObserver.js.map