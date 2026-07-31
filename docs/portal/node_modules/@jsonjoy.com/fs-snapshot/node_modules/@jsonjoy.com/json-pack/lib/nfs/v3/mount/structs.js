"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MountExportNode = exports.MountGroupNode = exports.MountBody = exports.MountFhandle3 = void 0;
/**
 * MOUNT protocol structures (Appendix I)
 */
/**
 * File handle for MOUNT protocol (opaque data)
 */
class MountFhandle3 {
    constructor(data) {
        this.data = data;
    }
}
exports.MountFhandle3 = MountFhandle3;
/**
 * Mount entry body structure
 */
class MountBody {
    constructor(hostname, directory, next) {
        this.hostname = hostname;
        this.directory = directory;
        this.next = next;
    }
}
exports.MountBody = MountBody;
/**
 * Group node for EXPORT
 */
class MountGroupNode {
    constructor(name, next) {
        this.name = name;
        this.next = next;
    }
}
exports.MountGroupNode = MountGroupNode;
/**
 * Export node structure
 */
class MountExportNode {
    constructor(dir, groups, next) {
        this.dir = dir;
        this.groups = groups;
        this.next = next;
    }
}
exports.MountExportNode = MountExportNode;
//# sourceMappingURL=structs.js.map