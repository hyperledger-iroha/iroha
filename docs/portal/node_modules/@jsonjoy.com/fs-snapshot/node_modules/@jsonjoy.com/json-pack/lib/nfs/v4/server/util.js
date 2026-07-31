"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.getOpNameFromRequest = exports.getOpName = exports.getProcName = exports.toHex = void 0;
const tslib_1 = require("tslib");
const msg = tslib_1.__importStar(require("../messages"));
const toHex = (buffer) => {
    return Array.from(buffer)
        .map((byte) => byte.toString(16).padStart(2, '0'))
        .join('');
};
exports.toHex = toHex;
const getProcName = (proc) => {
    switch (proc) {
        case 0 /* Nfsv4Proc.NULL */:
            return 'NULL';
        case 1 /* Nfsv4Proc.COMPOUND */:
            return 'COMPOUND';
    }
    return 'UNKNOWN(' + proc + ')';
};
exports.getProcName = getProcName;
const getOpName = (op) => {
    switch (op) {
        case 3 /* Nfsv4Op.ACCESS */:
            return 'ACCESS';
        case 4 /* Nfsv4Op.CLOSE */:
            return 'CLOSE';
        case 5 /* Nfsv4Op.COMMIT */:
            return 'COMMIT';
        case 6 /* Nfsv4Op.CREATE */:
            return 'CREATE';
        case 7 /* Nfsv4Op.DELEGPURGE */:
            return 'DELEGPURGE';
        case 8 /* Nfsv4Op.DELEGRETURN */:
            return 'DELEGRETURN';
        case 9 /* Nfsv4Op.GETATTR */:
            return 'GETATTR';
        case 10 /* Nfsv4Op.GETFH */:
            return 'GETFH';
        case 11 /* Nfsv4Op.LINK */:
            return 'LINK';
        case 12 /* Nfsv4Op.LOCK */:
            return 'LOCK';
        case 13 /* Nfsv4Op.LOCKT */:
            return 'LOCKT';
        case 14 /* Nfsv4Op.LOCKU */:
            return 'LOCKU';
        case 15 /* Nfsv4Op.LOOKUP */:
            return 'LOOKUP';
        case 16 /* Nfsv4Op.LOOKUPP */:
            return 'LOOKUPP';
        case 17 /* Nfsv4Op.NVERIFY */:
            return 'NVERIFY';
        case 18 /* Nfsv4Op.OPEN */:
            return 'OPEN';
        case 19 /* Nfsv4Op.OPENATTR */:
            return 'OPENATTR';
        case 20 /* Nfsv4Op.OPEN_CONFIRM */:
            return 'OPEN_CONFIRM';
        case 21 /* Nfsv4Op.OPEN_DOWNGRADE */:
            return 'OPEN_DOWNGRADE';
        case 22 /* Nfsv4Op.PUTFH */:
            return 'PUTFH';
        case 23 /* Nfsv4Op.PUTPUBFH */:
            return 'PUTPUBFH';
        case 24 /* Nfsv4Op.PUTROOTFH */:
            return 'PUTROOTFH';
        case 25 /* Nfsv4Op.READ */:
            return 'READ';
        case 26 /* Nfsv4Op.READDIR */:
            return 'READDIR';
        case 27 /* Nfsv4Op.READLINK */:
            return 'READLINK';
        case 28 /* Nfsv4Op.REMOVE */:
            return 'REMOVE';
        case 29 /* Nfsv4Op.RENAME */:
            return 'RENAME';
        case 30 /* Nfsv4Op.RENEW */:
            return 'RENEW';
        case 31 /* Nfsv4Op.RESTOREFH */:
            return 'RESTOREFH';
        case 32 /* Nfsv4Op.SAVEFH */:
            return 'SAVEFH';
        case 33 /* Nfsv4Op.SECINFO */:
            return 'SECINFO';
        case 34 /* Nfsv4Op.SETATTR */:
            return 'SETATTR';
        case 35 /* Nfsv4Op.SETCLIENTID */:
            return 'SETCLIENTID';
        case 36 /* Nfsv4Op.SETCLIENTID_CONFIRM */:
            return 'SETCLIENTID_CONFIRM';
        case 37 /* Nfsv4Op.VERIFY */:
            return 'VERIFY';
        case 38 /* Nfsv4Op.WRITE */:
            return 'WRITE';
        case 39 /* Nfsv4Op.RELEASE_LOCKOWNER */:
            return 'RELEASE_LOCKOWNER';
        case 10044 /* Nfsv4Op.ILLEGAL */:
            return 'ILLEGAL';
    }
    return 'UNKNOWN(' + op + ')';
};
exports.getOpName = getOpName;
const getOpNameFromRequest = (op) => {
    if (op instanceof msg.Nfsv4AccessRequest)
        return 'ACCESS';
    if (op instanceof msg.Nfsv4CloseRequest)
        return 'CLOSE';
    if (op instanceof msg.Nfsv4CommitRequest)
        return 'COMMIT';
    if (op instanceof msg.Nfsv4CreateRequest)
        return 'CREATE';
    if (op instanceof msg.Nfsv4DelegpurgeRequest)
        return 'DELEGPURGE';
    if (op instanceof msg.Nfsv4DelegreturnRequest)
        return 'DELEGRETURN';
    if (op instanceof msg.Nfsv4GetattrRequest)
        return 'GETATTR';
    if (op instanceof msg.Nfsv4GetfhRequest)
        return 'GETFH';
    if (op instanceof msg.Nfsv4LinkRequest)
        return 'LINK';
    if (op instanceof msg.Nfsv4LockRequest)
        return 'LOCK';
    if (op instanceof msg.Nfsv4LocktRequest)
        return 'LOCKT';
    if (op instanceof msg.Nfsv4LockuRequest)
        return 'LOCKU';
    if (op instanceof msg.Nfsv4LookupRequest)
        return 'LOOKUP';
    if (op instanceof msg.Nfsv4LookuppRequest)
        return 'LOOKUPP';
    if (op instanceof msg.Nfsv4NverifyRequest)
        return 'NVERIFY';
    if (op instanceof msg.Nfsv4OpenRequest)
        return 'OPEN';
    if (op instanceof msg.Nfsv4OpenattrRequest)
        return 'OPENATTR';
    if (op instanceof msg.Nfsv4OpenConfirmRequest)
        return 'OPEN_CONFIRM';
    if (op instanceof msg.Nfsv4OpenDowngradeRequest)
        return 'OPEN_DOWNGRADE';
    if (op instanceof msg.Nfsv4PutfhRequest)
        return 'PUTFH';
    if (op instanceof msg.Nfsv4PutpubfhRequest)
        return 'PUTPUBFH';
    if (op instanceof msg.Nfsv4PutrootfhRequest)
        return 'PUTROOTFH';
    if (op instanceof msg.Nfsv4ReadRequest)
        return 'READ';
    if (op instanceof msg.Nfsv4ReaddirRequest)
        return 'READDIR';
    if (op instanceof msg.Nfsv4ReadlinkRequest)
        return 'READLINK';
    if (op instanceof msg.Nfsv4RemoveRequest)
        return 'REMOVE';
    if (op instanceof msg.Nfsv4RenameRequest)
        return 'RENAME';
    if (op instanceof msg.Nfsv4RenewRequest)
        return 'RENEW';
    if (op instanceof msg.Nfsv4RestorefhRequest)
        return 'RESTOREFH';
    if (op instanceof msg.Nfsv4SavefhRequest)
        return 'SAVEFH';
    if (op instanceof msg.Nfsv4SecinfoRequest)
        return 'SECINFO';
    if (op instanceof msg.Nfsv4SetattrRequest)
        return 'SETATTR';
    if (op instanceof msg.Nfsv4SetclientidRequest)
        return 'SETCLIENTID';
    if (op instanceof msg.Nfsv4SetclientidConfirmRequest)
        return 'SETCLIENTID_CONFIRM';
    if (op instanceof msg.Nfsv4VerifyRequest)
        return 'VERIFY';
    if (op instanceof msg.Nfsv4WriteRequest)
        return 'WRITE';
    if (op instanceof msg.Nfsv4ReleaseLockOwnerRequest)
        return 'RELEASE_LOCKOWNER';
    if (op instanceof msg.Nfsv4IllegalRequest)
        return 'ILLEGAL';
    return 'UNKNOWN';
};
exports.getOpNameFromRequest = getOpNameFromRequest;
//# sourceMappingURL=util.js.map