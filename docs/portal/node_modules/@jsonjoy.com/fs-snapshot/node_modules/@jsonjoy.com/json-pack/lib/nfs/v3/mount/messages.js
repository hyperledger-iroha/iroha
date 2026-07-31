"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MountExportResponse = exports.MountExportRequest = exports.MountUmntallRequest = exports.MountUmntRequest = exports.MountDumpResponse = exports.MountDumpRequest = exports.MountMntResponse = exports.MountMntResOk = exports.MountMntRequest = void 0;
/**
 * MNT request
 */
class MountMntRequest {
    constructor(dirpath) {
        this.dirpath = dirpath;
    }
}
exports.MountMntRequest = MountMntRequest;
/**
 * MNT response - success case
 */
class MountMntResOk {
    constructor(fhandle, authFlavors) {
        this.fhandle = fhandle;
        this.authFlavors = authFlavors;
    }
}
exports.MountMntResOk = MountMntResOk;
/**
 * MNT response
 */
class MountMntResponse {
    constructor(status, mountinfo) {
        this.status = status;
        this.mountinfo = mountinfo;
    }
}
exports.MountMntResponse = MountMntResponse;
/**
 * DUMP request (void - no arguments)
 */
class MountDumpRequest {
}
exports.MountDumpRequest = MountDumpRequest;
/**
 * DUMP response
 */
class MountDumpResponse {
    constructor(mountlist) {
        this.mountlist = mountlist;
    }
}
exports.MountDumpResponse = MountDumpResponse;
/**
 * UMNT request
 */
class MountUmntRequest {
    constructor(dirpath) {
        this.dirpath = dirpath;
    }
}
exports.MountUmntRequest = MountUmntRequest;
/**
 * UMNTALL request (void - no arguments)
 */
class MountUmntallRequest {
}
exports.MountUmntallRequest = MountUmntallRequest;
/**
 * EXPORT request (void - no arguments)
 */
class MountExportRequest {
}
exports.MountExportRequest = MountExportRequest;
/**
 * EXPORT response
 */
class MountExportResponse {
    constructor(exports) {
        this.exports = exports;
    }
}
exports.MountExportResponse = MountExportResponse;
//# sourceMappingURL=messages.js.map