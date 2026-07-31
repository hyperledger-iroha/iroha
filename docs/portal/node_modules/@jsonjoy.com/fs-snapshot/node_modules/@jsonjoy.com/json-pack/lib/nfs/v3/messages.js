"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Nfsv3RmdirResFail = exports.Nfsv3RmdirResOk = exports.Nfsv3RmdirRequest = exports.Nfsv3RemoveResponse = exports.Nfsv3RemoveResFail = exports.Nfsv3RemoveResOk = exports.Nfsv3RemoveRequest = exports.Nfsv3MknodResponse = exports.Nfsv3MknodResFail = exports.Nfsv3MknodResOk = exports.Nfsv3MknodRequest = exports.Nfsv3SymlinkResponse = exports.Nfsv3SymlinkResFail = exports.Nfsv3SymlinkResOk = exports.Nfsv3SymlinkRequest = exports.Nfsv3MkdirResponse = exports.Nfsv3MkdirResFail = exports.Nfsv3MkdirResOk = exports.Nfsv3MkdirRequest = exports.Nfsv3CreateResponse = exports.Nfsv3CreateResFail = exports.Nfsv3CreateResOk = exports.Nfsv3CreateRequest = exports.Nfsv3WriteResponse = exports.Nfsv3WriteResFail = exports.Nfsv3WriteResOk = exports.Nfsv3WriteRequest = exports.Nfsv3ReadResponse = exports.Nfsv3ReadResFail = exports.Nfsv3ReadResOk = exports.Nfsv3ReadRequest = exports.Nfsv3ReadlinkResponse = exports.Nfsv3ReadlinkResFail = exports.Nfsv3ReadlinkResOk = exports.Nfsv3ReadlinkRequest = exports.Nfsv3AccessResponse = exports.Nfsv3AccessResFail = exports.Nfsv3AccessResOk = exports.Nfsv3AccessRequest = exports.Nfsv3LookupResponse = exports.Nfsv3LookupResFail = exports.Nfsv3LookupResOk = exports.Nfsv3LookupRequest = exports.Nfsv3SetattrResponse = exports.Nfsv3SetattrResFail = exports.Nfsv3SetattrResOk = exports.Nfsv3SetattrRequest = exports.Nfsv3GetattrResponse = exports.Nfsv3GetattrResOk = exports.Nfsv3GetattrRequest = void 0;
exports.Nfsv3CommitResponse = exports.Nfsv3CommitResFail = exports.Nfsv3CommitResOk = exports.Nfsv3CommitRequest = exports.Nfsv3PathconfResponse = exports.Nfsv3PathconfResFail = exports.Nfsv3PathconfResOk = exports.Nfsv3PathconfRequest = exports.Nfsv3FsinfoResponse = exports.Nfsv3FsinfoResFail = exports.Nfsv3FsinfoResOk = exports.Nfsv3FsinfoRequest = exports.Nfsv3FsstatResponse = exports.Nfsv3FsstatResFail = exports.Nfsv3FsstatResOk = exports.Nfsv3FsstatRequest = exports.Nfsv3ReaddirplusResponse = exports.Nfsv3ReaddirplusResFail = exports.Nfsv3ReaddirplusResOk = exports.Nfsv3ReaddirplusRequest = exports.Nfsv3ReaddirResponse = exports.Nfsv3ReaddirResFail = exports.Nfsv3ReaddirResOk = exports.Nfsv3ReaddirRequest = exports.Nfsv3LinkResponse = exports.Nfsv3LinkResFail = exports.Nfsv3LinkResOk = exports.Nfsv3LinkRequest = exports.Nfsv3RenameResponse = exports.Nfsv3RenameResFail = exports.Nfsv3RenameResOk = exports.Nfsv3RenameRequest = exports.Nfsv3RmdirResponse = void 0;
/**
 * GETATTR request
 */
class Nfsv3GetattrRequest {
    constructor(object) {
        this.object = object;
    }
}
exports.Nfsv3GetattrRequest = Nfsv3GetattrRequest;
/**
 * GETATTR response - success case
 */
class Nfsv3GetattrResOk {
    constructor(objAttributes) {
        this.objAttributes = objAttributes;
    }
}
exports.Nfsv3GetattrResOk = Nfsv3GetattrResOk;
/**
 * GETATTR response
 */
class Nfsv3GetattrResponse {
    constructor(status, resok) {
        this.status = status;
        this.resok = resok;
    }
}
exports.Nfsv3GetattrResponse = Nfsv3GetattrResponse;
/**
 * SETATTR request
 */
class Nfsv3SetattrRequest {
    constructor(object, newAttributes, guard) {
        this.object = object;
        this.newAttributes = newAttributes;
        this.guard = guard;
    }
}
exports.Nfsv3SetattrRequest = Nfsv3SetattrRequest;
/**
 * SETATTR response - success case
 */
class Nfsv3SetattrResOk {
    constructor(objWcc) {
        this.objWcc = objWcc;
    }
}
exports.Nfsv3SetattrResOk = Nfsv3SetattrResOk;
/**
 * SETATTR response - failure case
 */
class Nfsv3SetattrResFail {
    constructor(objWcc) {
        this.objWcc = objWcc;
    }
}
exports.Nfsv3SetattrResFail = Nfsv3SetattrResFail;
/**
 * SETATTR response
 */
class Nfsv3SetattrResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3SetattrResponse = Nfsv3SetattrResponse;
/**
 * LOOKUP request
 */
class Nfsv3LookupRequest {
    constructor(what) {
        this.what = what;
    }
}
exports.Nfsv3LookupRequest = Nfsv3LookupRequest;
/**
 * LOOKUP response - success case
 */
class Nfsv3LookupResOk {
    constructor(object, objAttributes, dirAttributes) {
        this.object = object;
        this.objAttributes = objAttributes;
        this.dirAttributes = dirAttributes;
    }
}
exports.Nfsv3LookupResOk = Nfsv3LookupResOk;
/**
 * LOOKUP response - failure case
 */
class Nfsv3LookupResFail {
    constructor(dirAttributes) {
        this.dirAttributes = dirAttributes;
    }
}
exports.Nfsv3LookupResFail = Nfsv3LookupResFail;
/**
 * LOOKUP response
 */
class Nfsv3LookupResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3LookupResponse = Nfsv3LookupResponse;
/**
 * ACCESS request
 */
class Nfsv3AccessRequest {
    constructor(object, access) {
        this.object = object;
        this.access = access;
    }
}
exports.Nfsv3AccessRequest = Nfsv3AccessRequest;
/**
 * ACCESS response - success case
 */
class Nfsv3AccessResOk {
    constructor(objAttributes, access) {
        this.objAttributes = objAttributes;
        this.access = access;
    }
}
exports.Nfsv3AccessResOk = Nfsv3AccessResOk;
/**
 * ACCESS response - failure case
 */
class Nfsv3AccessResFail {
    constructor(objAttributes) {
        this.objAttributes = objAttributes;
    }
}
exports.Nfsv3AccessResFail = Nfsv3AccessResFail;
/**
 * ACCESS response
 */
class Nfsv3AccessResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3AccessResponse = Nfsv3AccessResponse;
/**
 * READLINK request
 */
class Nfsv3ReadlinkRequest {
    constructor(symlink) {
        this.symlink = symlink;
    }
}
exports.Nfsv3ReadlinkRequest = Nfsv3ReadlinkRequest;
/**
 * READLINK response - success case
 */
class Nfsv3ReadlinkResOk {
    constructor(symlinkAttributes, data) {
        this.symlinkAttributes = symlinkAttributes;
        this.data = data;
    }
}
exports.Nfsv3ReadlinkResOk = Nfsv3ReadlinkResOk;
/**
 * READLINK response - failure case
 */
class Nfsv3ReadlinkResFail {
    constructor(symlinkAttributes) {
        this.symlinkAttributes = symlinkAttributes;
    }
}
exports.Nfsv3ReadlinkResFail = Nfsv3ReadlinkResFail;
/**
 * READLINK response
 */
class Nfsv3ReadlinkResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3ReadlinkResponse = Nfsv3ReadlinkResponse;
/**
 * READ request
 */
class Nfsv3ReadRequest {
    constructor(file, offset, count) {
        this.file = file;
        this.offset = offset;
        this.count = count;
    }
}
exports.Nfsv3ReadRequest = Nfsv3ReadRequest;
/**
 * READ response - success case
 */
class Nfsv3ReadResOk {
    constructor(fileAttributes, count, eof, data) {
        this.fileAttributes = fileAttributes;
        this.count = count;
        this.eof = eof;
        this.data = data;
    }
}
exports.Nfsv3ReadResOk = Nfsv3ReadResOk;
/**
 * READ response - failure case
 */
class Nfsv3ReadResFail {
    constructor(fileAttributes) {
        this.fileAttributes = fileAttributes;
    }
}
exports.Nfsv3ReadResFail = Nfsv3ReadResFail;
/**
 * READ response
 */
class Nfsv3ReadResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3ReadResponse = Nfsv3ReadResponse;
/**
 * WRITE request
 */
class Nfsv3WriteRequest {
    constructor(file, offset, count, stable, data) {
        this.file = file;
        this.offset = offset;
        this.count = count;
        this.stable = stable;
        this.data = data;
    }
}
exports.Nfsv3WriteRequest = Nfsv3WriteRequest;
/**
 * WRITE response - success case
 */
class Nfsv3WriteResOk {
    constructor(fileWcc, count, committed, verf) {
        this.fileWcc = fileWcc;
        this.count = count;
        this.committed = committed;
        this.verf = verf;
    }
}
exports.Nfsv3WriteResOk = Nfsv3WriteResOk;
/**
 * WRITE response - failure case
 */
class Nfsv3WriteResFail {
    constructor(fileWcc) {
        this.fileWcc = fileWcc;
    }
}
exports.Nfsv3WriteResFail = Nfsv3WriteResFail;
/**
 * WRITE response
 */
class Nfsv3WriteResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3WriteResponse = Nfsv3WriteResponse;
/**
 * CREATE request
 */
class Nfsv3CreateRequest {
    constructor(where, how) {
        this.where = where;
        this.how = how;
    }
}
exports.Nfsv3CreateRequest = Nfsv3CreateRequest;
/**
 * CREATE response - success case
 */
class Nfsv3CreateResOk {
    constructor(obj, objAttributes, dirWcc) {
        this.obj = obj;
        this.objAttributes = objAttributes;
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3CreateResOk = Nfsv3CreateResOk;
/**
 * CREATE response - failure case
 */
class Nfsv3CreateResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3CreateResFail = Nfsv3CreateResFail;
/**
 * CREATE response
 */
class Nfsv3CreateResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3CreateResponse = Nfsv3CreateResponse;
/**
 * MKDIR request
 */
class Nfsv3MkdirRequest {
    constructor(where, attributes) {
        this.where = where;
        this.attributes = attributes;
    }
}
exports.Nfsv3MkdirRequest = Nfsv3MkdirRequest;
/**
 * MKDIR response - success case
 */
class Nfsv3MkdirResOk {
    constructor(obj, objAttributes, dirWcc) {
        this.obj = obj;
        this.objAttributes = objAttributes;
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3MkdirResOk = Nfsv3MkdirResOk;
/**
 * MKDIR response - failure case
 */
class Nfsv3MkdirResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3MkdirResFail = Nfsv3MkdirResFail;
/**
 * MKDIR response
 */
class Nfsv3MkdirResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3MkdirResponse = Nfsv3MkdirResponse;
/**
 * SYMLINK request
 */
class Nfsv3SymlinkRequest {
    constructor(where, symlinkAttributes, symlinkData) {
        this.where = where;
        this.symlinkAttributes = symlinkAttributes;
        this.symlinkData = symlinkData;
    }
}
exports.Nfsv3SymlinkRequest = Nfsv3SymlinkRequest;
/**
 * SYMLINK response - success case
 */
class Nfsv3SymlinkResOk {
    constructor(obj, objAttributes, dirWcc) {
        this.obj = obj;
        this.objAttributes = objAttributes;
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3SymlinkResOk = Nfsv3SymlinkResOk;
/**
 * SYMLINK response - failure case
 */
class Nfsv3SymlinkResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3SymlinkResFail = Nfsv3SymlinkResFail;
/**
 * SYMLINK response
 */
class Nfsv3SymlinkResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3SymlinkResponse = Nfsv3SymlinkResponse;
/**
 * MKNOD request
 */
class Nfsv3MknodRequest {
    constructor(where, what) {
        this.where = where;
        this.what = what;
    }
}
exports.Nfsv3MknodRequest = Nfsv3MknodRequest;
/**
 * MKNOD response - success case
 */
class Nfsv3MknodResOk {
    constructor(obj, objAttributes, dirWcc) {
        this.obj = obj;
        this.objAttributes = objAttributes;
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3MknodResOk = Nfsv3MknodResOk;
/**
 * MKNOD response - failure case
 */
class Nfsv3MknodResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3MknodResFail = Nfsv3MknodResFail;
/**
 * MKNOD response
 */
class Nfsv3MknodResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3MknodResponse = Nfsv3MknodResponse;
/**
 * REMOVE request
 */
class Nfsv3RemoveRequest {
    constructor(object) {
        this.object = object;
    }
}
exports.Nfsv3RemoveRequest = Nfsv3RemoveRequest;
/**
 * REMOVE response - success case
 */
class Nfsv3RemoveResOk {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3RemoveResOk = Nfsv3RemoveResOk;
/**
 * REMOVE response - failure case
 */
class Nfsv3RemoveResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3RemoveResFail = Nfsv3RemoveResFail;
/**
 * REMOVE response
 */
class Nfsv3RemoveResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3RemoveResponse = Nfsv3RemoveResponse;
/**
 * RMDIR request
 */
class Nfsv3RmdirRequest {
    constructor(object) {
        this.object = object;
    }
}
exports.Nfsv3RmdirRequest = Nfsv3RmdirRequest;
/**
 * RMDIR response - success case
 */
class Nfsv3RmdirResOk {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3RmdirResOk = Nfsv3RmdirResOk;
/**
 * RMDIR response - failure case
 */
class Nfsv3RmdirResFail {
    constructor(dirWcc) {
        this.dirWcc = dirWcc;
    }
}
exports.Nfsv3RmdirResFail = Nfsv3RmdirResFail;
/**
 * RMDIR response
 */
class Nfsv3RmdirResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3RmdirResponse = Nfsv3RmdirResponse;
/**
 * RENAME request
 */
class Nfsv3RenameRequest {
    constructor(from, to) {
        this.from = from;
        this.to = to;
    }
}
exports.Nfsv3RenameRequest = Nfsv3RenameRequest;
/**
 * RENAME response - success case
 */
class Nfsv3RenameResOk {
    constructor(fromDirWcc, toDirWcc) {
        this.fromDirWcc = fromDirWcc;
        this.toDirWcc = toDirWcc;
    }
}
exports.Nfsv3RenameResOk = Nfsv3RenameResOk;
/**
 * RENAME response - failure case
 */
class Nfsv3RenameResFail {
    constructor(fromDirWcc, toDirWcc) {
        this.fromDirWcc = fromDirWcc;
        this.toDirWcc = toDirWcc;
    }
}
exports.Nfsv3RenameResFail = Nfsv3RenameResFail;
/**
 * RENAME response
 */
class Nfsv3RenameResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3RenameResponse = Nfsv3RenameResponse;
/**
 * LINK request
 */
class Nfsv3LinkRequest {
    constructor(file, link) {
        this.file = file;
        this.link = link;
    }
}
exports.Nfsv3LinkRequest = Nfsv3LinkRequest;
/**
 * LINK response - success case
 */
class Nfsv3LinkResOk {
    constructor(fileAttributes, linkDirWcc) {
        this.fileAttributes = fileAttributes;
        this.linkDirWcc = linkDirWcc;
    }
}
exports.Nfsv3LinkResOk = Nfsv3LinkResOk;
/**
 * LINK response - failure case
 */
class Nfsv3LinkResFail {
    constructor(fileAttributes, linkDirWcc) {
        this.fileAttributes = fileAttributes;
        this.linkDirWcc = linkDirWcc;
    }
}
exports.Nfsv3LinkResFail = Nfsv3LinkResFail;
/**
 * LINK response
 */
class Nfsv3LinkResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3LinkResponse = Nfsv3LinkResponse;
/**
 * READDIR request
 */
class Nfsv3ReaddirRequest {
    constructor(dir, cookie, cookieverf, count) {
        this.dir = dir;
        this.cookie = cookie;
        this.cookieverf = cookieverf;
        this.count = count;
    }
}
exports.Nfsv3ReaddirRequest = Nfsv3ReaddirRequest;
/**
 * READDIR response - success case
 */
class Nfsv3ReaddirResOk {
    constructor(dirAttributes, cookieverf, reply) {
        this.dirAttributes = dirAttributes;
        this.cookieverf = cookieverf;
        this.reply = reply;
    }
}
exports.Nfsv3ReaddirResOk = Nfsv3ReaddirResOk;
/**
 * READDIR response - failure case
 */
class Nfsv3ReaddirResFail {
    constructor(dirAttributes) {
        this.dirAttributes = dirAttributes;
    }
}
exports.Nfsv3ReaddirResFail = Nfsv3ReaddirResFail;
/**
 * READDIR response
 */
class Nfsv3ReaddirResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3ReaddirResponse = Nfsv3ReaddirResponse;
/**
 * READDIRPLUS request
 */
class Nfsv3ReaddirplusRequest {
    constructor(dir, cookie, cookieverf, dircount, maxcount) {
        this.dir = dir;
        this.cookie = cookie;
        this.cookieverf = cookieverf;
        this.dircount = dircount;
        this.maxcount = maxcount;
    }
}
exports.Nfsv3ReaddirplusRequest = Nfsv3ReaddirplusRequest;
/**
 * READDIRPLUS response - success case
 */
class Nfsv3ReaddirplusResOk {
    constructor(dirAttributes, cookieverf, reply) {
        this.dirAttributes = dirAttributes;
        this.cookieverf = cookieverf;
        this.reply = reply;
    }
}
exports.Nfsv3ReaddirplusResOk = Nfsv3ReaddirplusResOk;
/**
 * READDIRPLUS response - failure case
 */
class Nfsv3ReaddirplusResFail {
    constructor(dirAttributes) {
        this.dirAttributes = dirAttributes;
    }
}
exports.Nfsv3ReaddirplusResFail = Nfsv3ReaddirplusResFail;
/**
 * READDIRPLUS response
 */
class Nfsv3ReaddirplusResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3ReaddirplusResponse = Nfsv3ReaddirplusResponse;
/**
 * FSSTAT request
 */
class Nfsv3FsstatRequest {
    constructor(fsroot) {
        this.fsroot = fsroot;
    }
}
exports.Nfsv3FsstatRequest = Nfsv3FsstatRequest;
/**
 * FSSTAT response - success case
 */
class Nfsv3FsstatResOk {
    constructor(objAttributes, tbytes, fbytes, abytes, tfiles, ffiles, afiles, invarsec) {
        this.objAttributes = objAttributes;
        this.tbytes = tbytes;
        this.fbytes = fbytes;
        this.abytes = abytes;
        this.tfiles = tfiles;
        this.ffiles = ffiles;
        this.afiles = afiles;
        this.invarsec = invarsec;
    }
}
exports.Nfsv3FsstatResOk = Nfsv3FsstatResOk;
/**
 * FSSTAT response - failure case
 */
class Nfsv3FsstatResFail {
    constructor(objAttributes) {
        this.objAttributes = objAttributes;
    }
}
exports.Nfsv3FsstatResFail = Nfsv3FsstatResFail;
/**
 * FSSTAT response
 */
class Nfsv3FsstatResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3FsstatResponse = Nfsv3FsstatResponse;
/**
 * FSINFO request
 */
class Nfsv3FsinfoRequest {
    constructor(fsroot) {
        this.fsroot = fsroot;
    }
}
exports.Nfsv3FsinfoRequest = Nfsv3FsinfoRequest;
/**
 * FSINFO response - success case
 */
class Nfsv3FsinfoResOk {
    constructor(objAttributes, rtmax, rtpref, rtmult, wtmax, wtpref, wtmult, dtpref, maxfilesize, timeDelta, properties) {
        this.objAttributes = objAttributes;
        this.rtmax = rtmax;
        this.rtpref = rtpref;
        this.rtmult = rtmult;
        this.wtmax = wtmax;
        this.wtpref = wtpref;
        this.wtmult = wtmult;
        this.dtpref = dtpref;
        this.maxfilesize = maxfilesize;
        this.timeDelta = timeDelta;
        this.properties = properties;
    }
}
exports.Nfsv3FsinfoResOk = Nfsv3FsinfoResOk;
/**
 * FSINFO response - failure case
 */
class Nfsv3FsinfoResFail {
    constructor(objAttributes) {
        this.objAttributes = objAttributes;
    }
}
exports.Nfsv3FsinfoResFail = Nfsv3FsinfoResFail;
/**
 * FSINFO response
 */
class Nfsv3FsinfoResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3FsinfoResponse = Nfsv3FsinfoResponse;
/**
 * PATHCONF request
 */
class Nfsv3PathconfRequest {
    constructor(object) {
        this.object = object;
    }
}
exports.Nfsv3PathconfRequest = Nfsv3PathconfRequest;
/**
 * PATHCONF response - success case
 */
class Nfsv3PathconfResOk {
    constructor(objAttributes, linkmax, namemax, noTrunc, chownRestricted, caseInsensitive, casePreserving) {
        this.objAttributes = objAttributes;
        this.linkmax = linkmax;
        this.namemax = namemax;
        this.noTrunc = noTrunc;
        this.chownRestricted = chownRestricted;
        this.caseInsensitive = caseInsensitive;
        this.casePreserving = casePreserving;
    }
}
exports.Nfsv3PathconfResOk = Nfsv3PathconfResOk;
/**
 * PATHCONF response - failure case
 */
class Nfsv3PathconfResFail {
    constructor(objAttributes) {
        this.objAttributes = objAttributes;
    }
}
exports.Nfsv3PathconfResFail = Nfsv3PathconfResFail;
/**
 * PATHCONF response
 */
class Nfsv3PathconfResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3PathconfResponse = Nfsv3PathconfResponse;
/**
 * COMMIT request
 */
class Nfsv3CommitRequest {
    constructor(file, offset, count) {
        this.file = file;
        this.offset = offset;
        this.count = count;
    }
}
exports.Nfsv3CommitRequest = Nfsv3CommitRequest;
/**
 * COMMIT response - success case
 */
class Nfsv3CommitResOk {
    constructor(fileWcc, verf) {
        this.fileWcc = fileWcc;
        this.verf = verf;
    }
}
exports.Nfsv3CommitResOk = Nfsv3CommitResOk;
/**
 * COMMIT response - failure case
 */
class Nfsv3CommitResFail {
    constructor(fileWcc) {
        this.fileWcc = fileWcc;
    }
}
exports.Nfsv3CommitResFail = Nfsv3CommitResFail;
/**
 * COMMIT response
 */
class Nfsv3CommitResponse {
    constructor(status, resok, resfail) {
        this.status = status;
        this.resok = resok;
        this.resfail = resfail;
    }
}
exports.Nfsv3CommitResponse = Nfsv3CommitResponse;
//# sourceMappingURL=messages.js.map