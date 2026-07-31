"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.FileLockManager = void 0;
class FileLockManager {
    constructor() {
        this.locks = new Map();
    }
    acquireLock(path) {
        if (this.locks.get(path)) {
            return false;
        }
        this.locks.set(path, true);
        return true;
    }
    releaseLock(path) {
        this.locks.delete(path);
    }
    isLocked(path) {
        return this.locks.get(path) ?? false;
    }
    clear() {
        this.locks.clear();
    }
}
exports.FileLockManager = FileLockManager;
//# sourceMappingURL=FileLockManager.js.map