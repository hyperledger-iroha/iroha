"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.formatNfsv4CompoundResponse = exports.formatNfsv4CompoundRequest = exports.formatNfsv4Response = exports.formatNfsv4Request = exports.formatNfsv4Bitmap = exports.formatNfsv4Mode = exports.formatNfsv4Access = exports.formatNfsv4LockType = exports.formatNfsv4DelegType = exports.formatNfsv4OpenClaimType = exports.formatNfsv4OpenDeny = exports.formatNfsv4OpenAccess = exports.formatNfsv4OpenFlags = exports.formatNfsv4CreateMode = exports.formatNfsv4StableHow = exports.formatNfsv4TimeHow = exports.formatNfsv4FType = exports.formatNfsv4Attr = exports.formatNfsv4Op = exports.formatNfsv4Stat = void 0;
const tslib_1 = require("tslib");
const msg = tslib_1.__importStar(require("./messages"));
const attributes_1 = require("./attributes");
const printTree_1 = require("tree-dump/lib/printTree");
const formatNfsv4Stat = (stat) => {
    switch (stat) {
        case 0 /* constants.Nfsv4Stat.NFS4_OK */:
            return 'NFS4_OK';
        case 1 /* constants.Nfsv4Stat.NFS4ERR_PERM */:
            return 'NFS4ERR_PERM';
        case 2 /* constants.Nfsv4Stat.NFS4ERR_NOENT */:
            return 'NFS4ERR_NOENT';
        case 5 /* constants.Nfsv4Stat.NFS4ERR_IO */:
            return 'NFS4ERR_IO';
        case 6 /* constants.Nfsv4Stat.NFS4ERR_NXIO */:
            return 'NFS4ERR_NXIO';
        case 13 /* constants.Nfsv4Stat.NFS4ERR_ACCESS */:
            return 'NFS4ERR_ACCESS';
        case 17 /* constants.Nfsv4Stat.NFS4ERR_EXIST */:
            return 'NFS4ERR_EXIST';
        case 18 /* constants.Nfsv4Stat.NFS4ERR_XDEV */:
            return 'NFS4ERR_XDEV';
        case 20 /* constants.Nfsv4Stat.NFS4ERR_NOTDIR */:
            return 'NFS4ERR_NOTDIR';
        case 21 /* constants.Nfsv4Stat.NFS4ERR_ISDIR */:
            return 'NFS4ERR_ISDIR';
        case 22 /* constants.Nfsv4Stat.NFS4ERR_INVAL */:
            return 'NFS4ERR_INVAL';
        case 27 /* constants.Nfsv4Stat.NFS4ERR_FBIG */:
            return 'NFS4ERR_FBIG';
        case 28 /* constants.Nfsv4Stat.NFS4ERR_NOSPC */:
            return 'NFS4ERR_NOSPC';
        case 30 /* constants.Nfsv4Stat.NFS4ERR_ROFS */:
            return 'NFS4ERR_ROFS';
        case 31 /* constants.Nfsv4Stat.NFS4ERR_MLINK */:
            return 'NFS4ERR_MLINK';
        case 63 /* constants.Nfsv4Stat.NFS4ERR_NAMETOOLONG */:
            return 'NFS4ERR_NAMETOOLONG';
        case 66 /* constants.Nfsv4Stat.NFS4ERR_NOTEMPTY */:
            return 'NFS4ERR_NOTEMPTY';
        case 69 /* constants.Nfsv4Stat.NFS4ERR_DQUOT */:
            return 'NFS4ERR_DQUOT';
        case 70 /* constants.Nfsv4Stat.NFS4ERR_STALE */:
            return 'NFS4ERR_STALE';
        case 10001 /* constants.Nfsv4Stat.NFS4ERR_BADHANDLE */:
            return 'NFS4ERR_BADHANDLE';
        case 10003 /* constants.Nfsv4Stat.NFS4ERR_BAD_COOKIE */:
            return 'NFS4ERR_BAD_COOKIE';
        case 10004 /* constants.Nfsv4Stat.NFS4ERR_NOTSUPP */:
            return 'NFS4ERR_NOTSUPP';
        case 10005 /* constants.Nfsv4Stat.NFS4ERR_TOOSMALL */:
            return 'NFS4ERR_TOOSMALL';
        case 10006 /* constants.Nfsv4Stat.NFS4ERR_SERVERFAULT */:
            return 'NFS4ERR_SERVERFAULT';
        case 10007 /* constants.Nfsv4Stat.NFS4ERR_BADTYPE */:
            return 'NFS4ERR_BADTYPE';
        case 10008 /* constants.Nfsv4Stat.NFS4ERR_DELAY */:
            return 'NFS4ERR_DELAY';
        case 10009 /* constants.Nfsv4Stat.NFS4ERR_SAME */:
            return 'NFS4ERR_SAME';
        case 10010 /* constants.Nfsv4Stat.NFS4ERR_DENIED */:
            return 'NFS4ERR_DENIED';
        case 10011 /* constants.Nfsv4Stat.NFS4ERR_EXPIRED */:
            return 'NFS4ERR_EXPIRED';
        case 10012 /* constants.Nfsv4Stat.NFS4ERR_LOCKED */:
            return 'NFS4ERR_LOCKED';
        case 10013 /* constants.Nfsv4Stat.NFS4ERR_GRACE */:
            return 'NFS4ERR_GRACE';
        case 10014 /* constants.Nfsv4Stat.NFS4ERR_FHEXPIRED */:
            return 'NFS4ERR_FHEXPIRED';
        case 10015 /* constants.Nfsv4Stat.NFS4ERR_SHARE_DENIED */:
            return 'NFS4ERR_SHARE_DENIED';
        case 10016 /* constants.Nfsv4Stat.NFS4ERR_WRONGSEC */:
            return 'NFS4ERR_WRONGSEC';
        case 10017 /* constants.Nfsv4Stat.NFS4ERR_CLID_INUSE */:
            return 'NFS4ERR_CLID_INUSE';
        case 10018 /* constants.Nfsv4Stat.NFS4ERR_RESOURCE */:
            return 'NFS4ERR_RESOURCE';
        case 10019 /* constants.Nfsv4Stat.NFS4ERR_MOVED */:
            return 'NFS4ERR_MOVED';
        case 10020 /* constants.Nfsv4Stat.NFS4ERR_NOFILEHANDLE */:
            return 'NFS4ERR_NOFILEHANDLE';
        case 10021 /* constants.Nfsv4Stat.NFS4ERR_MINOR_VERS_MISMATCH */:
            return 'NFS4ERR_MINOR_VERS_MISMATCH';
        case 10022 /* constants.Nfsv4Stat.NFS4ERR_STALE_CLIENTID */:
            return 'NFS4ERR_STALE_CLIENTID';
        case 10023 /* constants.Nfsv4Stat.NFS4ERR_STALE_STATEID */:
            return 'NFS4ERR_STALE_STATEID';
        case 10024 /* constants.Nfsv4Stat.NFS4ERR_OLD_STATEID */:
            return 'NFS4ERR_OLD_STATEID';
        case 10025 /* constants.Nfsv4Stat.NFS4ERR_BAD_STATEID */:
            return 'NFS4ERR_BAD_STATEID';
        case 10026 /* constants.Nfsv4Stat.NFS4ERR_BAD_SEQID */:
            return 'NFS4ERR_BAD_SEQID';
        case 10027 /* constants.Nfsv4Stat.NFS4ERR_NOT_SAME */:
            return 'NFS4ERR_NOT_SAME';
        case 10028 /* constants.Nfsv4Stat.NFS4ERR_LOCK_RANGE */:
            return 'NFS4ERR_LOCK_RANGE';
        case 10029 /* constants.Nfsv4Stat.NFS4ERR_SYMLINK */:
            return 'NFS4ERR_SYMLINK';
        case 10030 /* constants.Nfsv4Stat.NFS4ERR_RESTOREFH */:
            return 'NFS4ERR_RESTOREFH';
        case 10031 /* constants.Nfsv4Stat.NFS4ERR_LEASE_MOVED */:
            return 'NFS4ERR_LEASE_MOVED';
        case 10032 /* constants.Nfsv4Stat.NFS4ERR_ATTRNOTSUPP */:
            return 'NFS4ERR_ATTRNOTSUPP';
        case 10033 /* constants.Nfsv4Stat.NFS4ERR_NO_GRACE */:
            return 'NFS4ERR_NO_GRACE';
        case 10034 /* constants.Nfsv4Stat.NFS4ERR_RECLAIM_BAD */:
            return 'NFS4ERR_RECLAIM_BAD';
        case 10035 /* constants.Nfsv4Stat.NFS4ERR_RECLAIM_CONFLICT */:
            return 'NFS4ERR_RECLAIM_CONFLICT';
        case 10036 /* constants.Nfsv4Stat.NFS4ERR_BADXDR */:
            return 'NFS4ERR_BADXDR';
        case 10037 /* constants.Nfsv4Stat.NFS4ERR_LOCKS_HELD */:
            return 'NFS4ERR_LOCKS_HELD';
        case 10038 /* constants.Nfsv4Stat.NFS4ERR_OPENMODE */:
            return 'NFS4ERR_OPENMODE';
        case 10039 /* constants.Nfsv4Stat.NFS4ERR_BADOWNER */:
            return 'NFS4ERR_BADOWNER';
        case 10040 /* constants.Nfsv4Stat.NFS4ERR_BADCHAR */:
            return 'NFS4ERR_BADCHAR';
        case 10041 /* constants.Nfsv4Stat.NFS4ERR_BADNAME */:
            return 'NFS4ERR_BADNAME';
        case 10042 /* constants.Nfsv4Stat.NFS4ERR_BAD_RANGE */:
            return 'NFS4ERR_BAD_RANGE';
        case 10043 /* constants.Nfsv4Stat.NFS4ERR_LOCK_NOTSUPP */:
            return 'NFS4ERR_LOCK_NOTSUPP';
        case 10044 /* constants.Nfsv4Stat.NFS4ERR_OP_ILLEGAL */:
            return 'NFS4ERR_OP_ILLEGAL';
        case 10045 /* constants.Nfsv4Stat.NFS4ERR_DEADLOCK */:
            return 'NFS4ERR_DEADLOCK';
        case 10046 /* constants.Nfsv4Stat.NFS4ERR_FILE_OPEN */:
            return 'NFS4ERR_FILE_OPEN';
        case 10047 /* constants.Nfsv4Stat.NFS4ERR_ADMIN_REVOKED */:
            return 'NFS4ERR_ADMIN_REVOKED';
        case 10048 /* constants.Nfsv4Stat.NFS4ERR_CB_PATH_DOWN */:
            return 'NFS4ERR_CB_PATH_DOWN';
        default:
            return `Unknown(${stat})`;
    }
};
exports.formatNfsv4Stat = formatNfsv4Stat;
const formatNfsv4Op = (op) => {
    switch (op) {
        case 3 /* constants.Nfsv4Op.ACCESS */:
            return 'ACCESS';
        case 4 /* constants.Nfsv4Op.CLOSE */:
            return 'CLOSE';
        case 5 /* constants.Nfsv4Op.COMMIT */:
            return 'COMMIT';
        case 6 /* constants.Nfsv4Op.CREATE */:
            return 'CREATE';
        case 7 /* constants.Nfsv4Op.DELEGPURGE */:
            return 'DELEGPURGE';
        case 8 /* constants.Nfsv4Op.DELEGRETURN */:
            return 'DELEGRETURN';
        case 9 /* constants.Nfsv4Op.GETATTR */:
            return 'GETATTR';
        case 10 /* constants.Nfsv4Op.GETFH */:
            return 'GETFH';
        case 11 /* constants.Nfsv4Op.LINK */:
            return 'LINK';
        case 12 /* constants.Nfsv4Op.LOCK */:
            return 'LOCK';
        case 13 /* constants.Nfsv4Op.LOCKT */:
            return 'LOCKT';
        case 14 /* constants.Nfsv4Op.LOCKU */:
            return 'LOCKU';
        case 15 /* constants.Nfsv4Op.LOOKUP */:
            return 'LOOKUP';
        case 16 /* constants.Nfsv4Op.LOOKUPP */:
            return 'LOOKUPP';
        case 17 /* constants.Nfsv4Op.NVERIFY */:
            return 'NVERIFY';
        case 18 /* constants.Nfsv4Op.OPEN */:
            return 'OPEN';
        case 19 /* constants.Nfsv4Op.OPENATTR */:
            return 'OPENATTR';
        case 20 /* constants.Nfsv4Op.OPEN_CONFIRM */:
            return 'OPEN_CONFIRM';
        case 21 /* constants.Nfsv4Op.OPEN_DOWNGRADE */:
            return 'OPEN_DOWNGRADE';
        case 22 /* constants.Nfsv4Op.PUTFH */:
            return 'PUTFH';
        case 23 /* constants.Nfsv4Op.PUTPUBFH */:
            return 'PUTPUBFH';
        case 24 /* constants.Nfsv4Op.PUTROOTFH */:
            return 'PUTROOTFH';
        case 25 /* constants.Nfsv4Op.READ */:
            return 'READ';
        case 26 /* constants.Nfsv4Op.READDIR */:
            return 'READDIR';
        case 27 /* constants.Nfsv4Op.READLINK */:
            return 'READLINK';
        case 28 /* constants.Nfsv4Op.REMOVE */:
            return 'REMOVE';
        case 29 /* constants.Nfsv4Op.RENAME */:
            return 'RENAME';
        case 30 /* constants.Nfsv4Op.RENEW */:
            return 'RENEW';
        case 31 /* constants.Nfsv4Op.RESTOREFH */:
            return 'RESTOREFH';
        case 32 /* constants.Nfsv4Op.SAVEFH */:
            return 'SAVEFH';
        case 33 /* constants.Nfsv4Op.SECINFO */:
            return 'SECINFO';
        case 34 /* constants.Nfsv4Op.SETATTR */:
            return 'SETATTR';
        case 35 /* constants.Nfsv4Op.SETCLIENTID */:
            return 'SETCLIENTID';
        case 36 /* constants.Nfsv4Op.SETCLIENTID_CONFIRM */:
            return 'SETCLIENTID_CONFIRM';
        case 37 /* constants.Nfsv4Op.VERIFY */:
            return 'VERIFY';
        case 38 /* constants.Nfsv4Op.WRITE */:
            return 'WRITE';
        case 39 /* constants.Nfsv4Op.RELEASE_LOCKOWNER */:
            return 'RELEASE_LOCKOWNER';
        case 10044 /* constants.Nfsv4Op.ILLEGAL */:
            return 'ILLEGAL';
        default:
            return `Unknown(${op})`;
    }
};
exports.formatNfsv4Op = formatNfsv4Op;
const formatNfsv4Attr = (attr) => {
    switch (attr) {
        case 0 /* constants.Nfsv4Attr.FATTR4_SUPPORTED_ATTRS */:
            return 'supported_attrs';
        case 1 /* constants.Nfsv4Attr.FATTR4_TYPE */:
            return 'type';
        case 2 /* constants.Nfsv4Attr.FATTR4_FH_EXPIRE_TYPE */:
            return 'fh_expire_type';
        case 3 /* constants.Nfsv4Attr.FATTR4_CHANGE */:
            return 'change';
        case 4 /* constants.Nfsv4Attr.FATTR4_SIZE */:
            return 'size';
        case 5 /* constants.Nfsv4Attr.FATTR4_LINK_SUPPORT */:
            return 'link_support';
        case 6 /* constants.Nfsv4Attr.FATTR4_SYMLINK_SUPPORT */:
            return 'symlink_support';
        case 7 /* constants.Nfsv4Attr.FATTR4_NAMED_ATTR */:
            return 'named_attr';
        case 8 /* constants.Nfsv4Attr.FATTR4_FSID */:
            return 'fsid';
        case 9 /* constants.Nfsv4Attr.FATTR4_UNIQUE_HANDLES */:
            return 'unique_handles';
        case 10 /* constants.Nfsv4Attr.FATTR4_LEASE_TIME */:
            return 'lease_time';
        case 11 /* constants.Nfsv4Attr.FATTR4_RDATTR_ERROR */:
            return 'rdattr_error';
        case 12 /* constants.Nfsv4Attr.FATTR4_ACL */:
            return 'acl';
        case 13 /* constants.Nfsv4Attr.FATTR4_ACLSUPPORT */:
            return 'aclsupport';
        case 14 /* constants.Nfsv4Attr.FATTR4_ARCHIVE */:
            return 'archive';
        case 15 /* constants.Nfsv4Attr.FATTR4_CANSETTIME */:
            return 'can_set_time';
        case 16 /* constants.Nfsv4Attr.FATTR4_CASE_INSENSITIVE */:
            return 'case_insensitive';
        case 17 /* constants.Nfsv4Attr.FATTR4_CASE_PRESERVING */:
            return 'case_preserving';
        case 18 /* constants.Nfsv4Attr.FATTR4_CHOWN_RESTRICTED */:
            return 'chown_restricted';
        case 19 /* constants.Nfsv4Attr.FATTR4_FILEHANDLE */:
            return 'filehandle';
        case 20 /* constants.Nfsv4Attr.FATTR4_FILEID */:
            return 'fileid';
        case 21 /* constants.Nfsv4Attr.FATTR4_FILES_AVAIL */:
            return 'files_avail';
        case 22 /* constants.Nfsv4Attr.FATTR4_FILES_FREE */:
            return 'files_free';
        case 23 /* constants.Nfsv4Attr.FATTR4_FILES_TOTAL */:
            return 'files_total';
        case 24 /* constants.Nfsv4Attr.FATTR4_FS_LOCATIONS */:
            return 'fs_locations';
        case 25 /* constants.Nfsv4Attr.FATTR4_HIDDEN */:
            return 'hidden';
        case 26 /* constants.Nfsv4Attr.FATTR4_HOMOGENEOUS */:
            return 'homogeneous';
        case 27 /* constants.Nfsv4Attr.FATTR4_MAXFILESIZE */:
            return 'maxfilesize';
        case 28 /* constants.Nfsv4Attr.FATTR4_MAXLINK */:
            return 'maxlink';
        case 29 /* constants.Nfsv4Attr.FATTR4_MAXNAME */:
            return 'maxname';
        case 30 /* constants.Nfsv4Attr.FATTR4_MAXREAD */:
            return 'maxread';
        case 31 /* constants.Nfsv4Attr.FATTR4_MAXWRITE */:
            return 'maxwrite';
        case 32 /* constants.Nfsv4Attr.FATTR4_MIMETYPE */:
            return 'mimetype';
        case 33 /* constants.Nfsv4Attr.FATTR4_MODE */:
            return 'mode';
        case 34 /* constants.Nfsv4Attr.FATTR4_NO_TRUNC */:
            return 'no_trunc';
        case 35 /* constants.Nfsv4Attr.FATTR4_NUMLINKS */:
            return 'numlinks';
        case 36 /* constants.Nfsv4Attr.FATTR4_OWNER */:
            return 'owner';
        case 37 /* constants.Nfsv4Attr.FATTR4_OWNER_GROUP */:
            return 'owner_group';
        case 38 /* constants.Nfsv4Attr.FATTR4_QUOTA_AVAIL_HARD */:
            return 'quota_avail_hard';
        case 39 /* constants.Nfsv4Attr.FATTR4_QUOTA_AVAIL_SOFT */:
            return 'quota_avail_soft';
        case 40 /* constants.Nfsv4Attr.FATTR4_QUOTA_USED */:
            return 'quota_used';
        case 41 /* constants.Nfsv4Attr.FATTR4_RAWDEV */:
            return 'rawdev';
        case 42 /* constants.Nfsv4Attr.FATTR4_SPACE_AVAIL */:
            return 'space_avail';
        case 43 /* constants.Nfsv4Attr.FATTR4_SPACE_FREE */:
            return 'space_free';
        case 44 /* constants.Nfsv4Attr.FATTR4_SPACE_TOTAL */:
            return 'space_total';
        case 45 /* constants.Nfsv4Attr.FATTR4_SPACE_USED */:
            return 'space_used';
        case 46 /* constants.Nfsv4Attr.FATTR4_SYSTEM */:
            return 'system';
        case 47 /* constants.Nfsv4Attr.FATTR4_TIME_ACCESS */:
            return 'time_access';
        case 48 /* constants.Nfsv4Attr.FATTR4_TIME_ACCESS_SET */:
            return 'time_access_set';
        case 49 /* constants.Nfsv4Attr.FATTR4_TIME_BACKUP */:
            return 'time_backup';
        case 50 /* constants.Nfsv4Attr.FATTR4_TIME_CREATE */:
            return 'time_create';
        case 51 /* constants.Nfsv4Attr.FATTR4_TIME_DELTA */:
            return 'time_delta';
        case 52 /* constants.Nfsv4Attr.FATTR4_TIME_METADATA */:
            return 'time_metadata';
        case 53 /* constants.Nfsv4Attr.FATTR4_TIME_MODIFY */:
            return 'time_modify';
        case 54 /* constants.Nfsv4Attr.FATTR4_TIME_MODIFY_SET */:
            return 'time_modify_set';
        case 55 /* constants.Nfsv4Attr.FATTR4_MOUNTED_ON_FILEID */:
            return 'mounted_on_fileid';
        default:
            return `Unknown(${attr})`;
    }
};
exports.formatNfsv4Attr = formatNfsv4Attr;
const formatNfsv4FType = (ftype) => {
    switch (ftype) {
        case 1 /* constants.Nfsv4FType.NF4REG */:
            return 'NF4REG';
        case 2 /* constants.Nfsv4FType.NF4DIR */:
            return 'NF4DIR';
        case 3 /* constants.Nfsv4FType.NF4BLK */:
            return 'NF4BLK';
        case 4 /* constants.Nfsv4FType.NF4CHR */:
            return 'NF4CHR';
        case 5 /* constants.Nfsv4FType.NF4LNK */:
            return 'NF4LNK';
        case 6 /* constants.Nfsv4FType.NF4SOCK */:
            return 'NF4SOCK';
        case 7 /* constants.Nfsv4FType.NF4FIFO */:
            return 'NF4FIFO';
        case 8 /* constants.Nfsv4FType.NF4ATTRDIR */:
            return 'NF4ATTRDIR';
        case 9 /* constants.Nfsv4FType.NF4NAMEDATTR */:
            return 'NF4NAMEDATTR';
        default:
            return `Unknown(${ftype})`;
    }
};
exports.formatNfsv4FType = formatNfsv4FType;
const formatNfsv4TimeHow = (how) => {
    switch (how) {
        case 0 /* constants.Nfsv4TimeHow.SET_TO_SERVER_TIME4 */:
            return 'SET_TO_SERVER_TIME4';
        case 1 /* constants.Nfsv4TimeHow.SET_TO_CLIENT_TIME4 */:
            return 'SET_TO_CLIENT_TIME4';
        default:
            return `Unknown(${how})`;
    }
};
exports.formatNfsv4TimeHow = formatNfsv4TimeHow;
const formatNfsv4StableHow = (stable) => {
    switch (stable) {
        case 0 /* constants.Nfsv4StableHow.UNSTABLE4 */:
            return 'UNSTABLE4';
        case 1 /* constants.Nfsv4StableHow.DATA_SYNC4 */:
            return 'DATA_SYNC4';
        case 2 /* constants.Nfsv4StableHow.FILE_SYNC4 */:
            return 'FILE_SYNC4';
        default:
            return `Unknown(${stable})`;
    }
};
exports.formatNfsv4StableHow = formatNfsv4StableHow;
const formatNfsv4CreateMode = (mode) => {
    switch (mode) {
        case 0 /* constants.Nfsv4CreateMode.UNCHECKED4 */:
            return 'UNCHECKED4';
        case 1 /* constants.Nfsv4CreateMode.GUARDED4 */:
            return 'GUARDED4';
        case 2 /* constants.Nfsv4CreateMode.EXCLUSIVE4 */:
            return 'EXCLUSIVE4';
        default:
            return `Unknown(${mode})`;
    }
};
exports.formatNfsv4CreateMode = formatNfsv4CreateMode;
const formatNfsv4OpenFlags = (flags) => {
    switch (flags) {
        case 0 /* constants.Nfsv4OpenFlags.OPEN4_NOCREATE */:
            return 'OPEN4_NOCREATE';
        case 1 /* constants.Nfsv4OpenFlags.OPEN4_CREATE */:
            return 'OPEN4_CREATE';
        default:
            return `Unknown(${flags})`;
    }
};
exports.formatNfsv4OpenFlags = formatNfsv4OpenFlags;
const formatNfsv4OpenAccess = (access) => {
    switch (access) {
        case 1 /* constants.Nfsv4OpenAccess.OPEN4_SHARE_ACCESS_READ */:
            return 'OPEN4_SHARE_ACCESS_READ';
        case 2 /* constants.Nfsv4OpenAccess.OPEN4_SHARE_ACCESS_WRITE */:
            return 'OPEN4_SHARE_ACCESS_WRITE';
        case 3 /* constants.Nfsv4OpenAccess.OPEN4_SHARE_ACCESS_BOTH */:
            return 'OPEN4_SHARE_ACCESS_BOTH';
        default:
            return `Unknown(${access})`;
    }
};
exports.formatNfsv4OpenAccess = formatNfsv4OpenAccess;
const formatNfsv4OpenDeny = (deny) => {
    switch (deny) {
        case 0 /* constants.Nfsv4OpenDeny.OPEN4_SHARE_DENY_NONE */:
            return 'OPEN4_SHARE_DENY_NONE';
        case 1 /* constants.Nfsv4OpenDeny.OPEN4_SHARE_DENY_READ */:
            return 'OPEN4_SHARE_DENY_READ';
        case 2 /* constants.Nfsv4OpenDeny.OPEN4_SHARE_DENY_WRITE */:
            return 'OPEN4_SHARE_DENY_WRITE';
        case 3 /* constants.Nfsv4OpenDeny.OPEN4_SHARE_DENY_BOTH */:
            return 'OPEN4_SHARE_DENY_BOTH';
        default:
            return `Unknown(${deny})`;
    }
};
exports.formatNfsv4OpenDeny = formatNfsv4OpenDeny;
const formatNfsv4OpenClaimType = (claim) => {
    switch (claim) {
        case 0 /* constants.Nfsv4OpenClaimType.CLAIM_NULL */:
            return 'CLAIM_NULL';
        case 1 /* constants.Nfsv4OpenClaimType.CLAIM_PREVIOUS */:
            return 'CLAIM_PREVIOUS';
        case 2 /* constants.Nfsv4OpenClaimType.CLAIM_DELEGATE_CUR */:
            return 'CLAIM_DELEGATE_CUR';
        case 3 /* constants.Nfsv4OpenClaimType.CLAIM_DELEGATE_PREV */:
            return 'CLAIM_DELEGATE_PREV';
        default:
            return `Unknown(${claim})`;
    }
};
exports.formatNfsv4OpenClaimType = formatNfsv4OpenClaimType;
const formatNfsv4DelegType = (deleg) => {
    switch (deleg) {
        case 0 /* constants.Nfsv4DelegType.OPEN_DELEGATE_NONE */:
            return 'OPEN_DELEGATE_NONE';
        case 1 /* constants.Nfsv4DelegType.OPEN_DELEGATE_READ */:
            return 'OPEN_DELEGATE_READ';
        case 2 /* constants.Nfsv4DelegType.OPEN_DELEGATE_WRITE */:
            return 'OPEN_DELEGATE_WRITE';
        default:
            return `Unknown(${deleg})`;
    }
};
exports.formatNfsv4DelegType = formatNfsv4DelegType;
const formatNfsv4LockType = (locktype) => {
    switch (locktype) {
        case 1 /* constants.Nfsv4LockType.READ_LT */:
            return 'READ_LT';
        case 2 /* constants.Nfsv4LockType.WRITE_LT */:
            return 'WRITE_LT';
        case 3 /* constants.Nfsv4LockType.READW_LT */:
            return 'READW_LT';
        case 4 /* constants.Nfsv4LockType.WRITEW_LT */:
            return 'WRITEW_LT';
        default:
            return `Unknown(${locktype})`;
    }
};
exports.formatNfsv4LockType = formatNfsv4LockType;
const formatNfsv4Access = (access) => {
    const flags = [];
    if (access & 1 /* constants.Nfsv4Access.ACCESS4_READ */)
        flags.push('READ');
    if (access & 2 /* constants.Nfsv4Access.ACCESS4_LOOKUP */)
        flags.push('LOOKUP');
    if (access & 4 /* constants.Nfsv4Access.ACCESS4_MODIFY */)
        flags.push('MODIFY');
    if (access & 8 /* constants.Nfsv4Access.ACCESS4_EXTEND */)
        flags.push('EXTEND');
    if (access & 16 /* constants.Nfsv4Access.ACCESS4_DELETE */)
        flags.push('DELETE');
    if (access & 32 /* constants.Nfsv4Access.ACCESS4_EXECUTE */)
        flags.push('EXECUTE');
    return flags.length > 0 ? flags.join('|') : `0x${access.toString(16)}`;
};
exports.formatNfsv4Access = formatNfsv4Access;
const formatNfsv4Mode = (mode) => {
    const flags = [];
    if (mode & 2048 /* constants.Nfsv4Mode.MODE4_SUID */)
        flags.push('SUID');
    if (mode & 1024 /* constants.Nfsv4Mode.MODE4_SGID */)
        flags.push('SGID');
    if (mode & 512 /* constants.Nfsv4Mode.MODE4_SVTX */)
        flags.push('SVTX');
    if (mode & 256 /* constants.Nfsv4Mode.MODE4_RUSR */)
        flags.push('RUSR');
    if (mode & 128 /* constants.Nfsv4Mode.MODE4_WUSR */)
        flags.push('WUSR');
    if (mode & 64 /* constants.Nfsv4Mode.MODE4_XUSR */)
        flags.push('XUSR');
    if (mode & 32 /* constants.Nfsv4Mode.MODE4_RGRP */)
        flags.push('RGRP');
    if (mode & 16 /* constants.Nfsv4Mode.MODE4_WGRP */)
        flags.push('WGRP');
    if (mode & 8 /* constants.Nfsv4Mode.MODE4_XGRP */)
        flags.push('XGRP');
    if (mode & 4 /* constants.Nfsv4Mode.MODE4_ROTH */)
        flags.push('ROTH');
    if (mode & 2 /* constants.Nfsv4Mode.MODE4_WOTH */)
        flags.push('WOTH');
    if (mode & 1 /* constants.Nfsv4Mode.MODE4_XOTH */)
        flags.push('XOTH');
    const octal = mode.toString(8).padStart(4, '0');
    return flags.length > 0 ? `${octal} (${flags.join('|')})` : octal;
};
exports.formatNfsv4Mode = formatNfsv4Mode;
const formatNfsv4Bitmap = (bitmap) => {
    const attrs = [];
    const attrNums = (0, attributes_1.parseBitmask)(bitmap.mask);
    for (const num of attrNums)
        attrs.push((0, exports.formatNfsv4Attr)(num));
    return attrs.length > 0 ? `[${attrs.join(', ')}]` : '[]';
};
exports.formatNfsv4Bitmap = formatNfsv4Bitmap;
const formatBytes = (data, maxLen = 32) => {
    if (data.length === 0)
        return '[]';
    const hex = Array.from(data.slice(0, maxLen), (b) => b.toString(16).padStart(2, '0')).join(' ');
    return data.length > maxLen ? `[${hex}... (${data.length} bytes)]` : `[${hex}]`;
};
const formatStateid = (stateid, tab = '') => {
    return `Stateid { seqid = ${stateid.seqid}, other = ${formatBytes(stateid.other)} }`;
};
const formatFileHandle = (fh) => {
    return formatBytes(fh.data, 16);
};
const formatNfsv4Request = (req, tab = '') => {
    if (req instanceof msg.Nfsv4AccessRequest) {
        return `ACCESS access = ${(0, exports.formatNfsv4Access)(req.access)}`;
    }
    else if (req instanceof msg.Nfsv4CloseRequest) {
        return `CLOSE seqid = ${req.seqid}, stateid = ${formatStateid(req.openStateid, tab)}`;
    }
    else if (req instanceof msg.Nfsv4CommitRequest) {
        return `COMMIT offset = ${req.offset}, count = ${req.count}`;
    }
    else if (req instanceof msg.Nfsv4CreateRequest) {
        return `CREATE objtype = ${(0, exports.formatNfsv4FType)(req.objtype.type)}, objname = "${req.objname}"`;
    }
    else if (req instanceof msg.Nfsv4DelegpurgeRequest) {
        return `DELEGPURGE clientid = ${req.clientid}`;
    }
    else if (req instanceof msg.Nfsv4DelegreturnRequest) {
        return `DELEGRETURN stateid = ${formatStateid(req.delegStateid, tab)}`;
    }
    else if (req instanceof msg.Nfsv4GetattrRequest) {
        return `GETATTR attrs = ${(0, exports.formatNfsv4Bitmap)(req.attrRequest)}`;
    }
    else if (req instanceof msg.Nfsv4GetfhRequest) {
        return 'GETFH';
    }
    else if (req instanceof msg.Nfsv4LinkRequest) {
        return `LINK newname = "${req.newname}"`;
    }
    else if (req instanceof msg.Nfsv4LockRequest) {
        return `LOCK locktype = ${(0, exports.formatNfsv4LockType)(req.locktype)}, reclaim = ${req.reclaim}, offset = ${req.offset}, length = ${req.length}`;
    }
    else if (req instanceof msg.Nfsv4LocktRequest) {
        return `LOCKT locktype = ${(0, exports.formatNfsv4LockType)(req.locktype)}, offset = ${req.offset}, length = ${req.length}`;
    }
    else if (req instanceof msg.Nfsv4LockuRequest) {
        return `LOCKU locktype = ${(0, exports.formatNfsv4LockType)(req.locktype)}, seqid = ${req.seqid}, stateid = ${formatStateid(req.lockStateid, tab)}, offset = ${req.offset}, length = ${req.length}`;
    }
    else if (req instanceof msg.Nfsv4LookupRequest) {
        return `LOOKUP objname = "${req.objname}"`;
    }
    else if (req instanceof msg.Nfsv4LookuppRequest) {
        return 'LOOKUPP';
    }
    else if (req instanceof msg.Nfsv4NverifyRequest) {
        return `NVERIFY attrs = ${(0, exports.formatNfsv4Bitmap)(req.objAttributes.attrmask)}`;
    }
    else if (req instanceof msg.Nfsv4OpenRequest) {
        const createInfo = req.openhow.how ? `, createmode = ${(0, exports.formatNfsv4CreateMode)(req.openhow.how.mode)}` : '';
        return `OPEN seqid = ${req.seqid}, shareAccess = ${(0, exports.formatNfsv4OpenAccess)(req.shareAccess)}, shareDeny = ${(0, exports.formatNfsv4OpenDeny)(req.shareDeny)}, opentype = ${(0, exports.formatNfsv4OpenFlags)(req.openhow.opentype)}${createInfo}, claim = ${(0, exports.formatNfsv4OpenClaimType)(req.claim.claimType)}`;
    }
    else if (req instanceof msg.Nfsv4OpenattrRequest) {
        return `OPENATTR createdir = ${req.createdir}`;
    }
    else if (req instanceof msg.Nfsv4OpenConfirmRequest) {
        return `OPEN_CONFIRM stateid = ${formatStateid(req.openStateid, tab)}, seqid = ${req.seqid}`;
    }
    else if (req instanceof msg.Nfsv4OpenDowngradeRequest) {
        return `OPEN_DOWNGRADE stateid = ${formatStateid(req.openStateid, tab)}, seqid = ${req.seqid}, shareAccess = ${(0, exports.formatNfsv4OpenAccess)(req.shareAccess)}, shareDeny = ${(0, exports.formatNfsv4OpenDeny)(req.shareDeny)}`;
    }
    else if (req instanceof msg.Nfsv4PutfhRequest) {
        return `PUTFH fh = ${formatFileHandle(req.object)}`;
    }
    else if (req instanceof msg.Nfsv4PutpubfhRequest) {
        return 'PUTPUBFH';
    }
    else if (req instanceof msg.Nfsv4PutrootfhRequest) {
        return 'PUTROOTFH';
    }
    else if (req instanceof msg.Nfsv4ReadRequest) {
        return `READ stateid = ${formatStateid(req.stateid, tab)}, offset = ${req.offset}, count = ${req.count}`;
    }
    else if (req instanceof msg.Nfsv4ReaddirRequest) {
        return `READDIR cookie = ${req.cookie}, dircount = ${req.dircount}, maxcount = ${req.maxcount}, attrs = ${(0, exports.formatNfsv4Bitmap)(req.attrRequest)}`;
    }
    else if (req instanceof msg.Nfsv4ReadlinkRequest) {
        return 'READLINK';
    }
    else if (req instanceof msg.Nfsv4RemoveRequest) {
        return `REMOVE target = "${req.target}"`;
    }
    else if (req instanceof msg.Nfsv4RenameRequest) {
        return `RENAME oldname = "${req.oldname}", newname = "${req.newname}"`;
    }
    else if (req instanceof msg.Nfsv4RenewRequest) {
        return `RENEW clientid = ${req.clientid}`;
    }
    else if (req instanceof msg.Nfsv4RestorefhRequest) {
        return 'RESTOREFH';
    }
    else if (req instanceof msg.Nfsv4SavefhRequest) {
        return 'SAVEFH';
    }
    else if (req instanceof msg.Nfsv4SecinfoRequest) {
        return `SECINFO name = "${req.name}"`;
    }
    else if (req instanceof msg.Nfsv4SetattrRequest) {
        return `SETATTR stateid = ${formatStateid(req.stateid, tab)}, attrs = ${(0, exports.formatNfsv4Bitmap)(req.objAttributes.attrmask)}`;
    }
    else if (req instanceof msg.Nfsv4SetclientidRequest) {
        return `SETCLIENTID callbackIdent = ${req.callbackIdent}`;
    }
    else if (req instanceof msg.Nfsv4SetclientidConfirmRequest) {
        return `SETCLIENTID_CONFIRM clientid = ${req.clientid}`;
    }
    else if (req instanceof msg.Nfsv4VerifyRequest) {
        return `VERIFY attrs = ${(0, exports.formatNfsv4Bitmap)(req.objAttributes.attrmask)}`;
    }
    else if (req instanceof msg.Nfsv4WriteRequest) {
        return `WRITE stateid = ${formatStateid(req.stateid, tab)}, offset = ${req.offset}, stable = ${(0, exports.formatNfsv4StableHow)(req.stable)}, length = ${req.data.length}`;
    }
    else if (req instanceof msg.Nfsv4ReleaseLockOwnerRequest) {
        return 'RELEASE_LOCKOWNER';
    }
    else if (req instanceof msg.Nfsv4IllegalRequest) {
        return 'ILLEGAL';
    }
    return 'Unknown Request';
};
exports.formatNfsv4Request = formatNfsv4Request;
const formatNfsv4Response = (res, tab = '') => {
    if (res instanceof msg.Nfsv4AccessResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `ACCESS (${(0, exports.formatNfsv4Stat)(res.status)}) supported = ${(0, exports.formatNfsv4Access)(res.resok.supported)}, access = ${(0, exports.formatNfsv4Access)(res.resok.access)}`;
        }
        return `ACCESS (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4CloseResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `stateid = ${formatStateid(res.resok.openStateid, tab)}`);
        }
        return `CLOSE (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4CommitResponse) {
        return `COMMIT (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4CreateResponse) {
        return `CREATE (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4DelegpurgeResponse) {
        return `DELEGPURGE (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4DelegreturnResponse) {
        return `DELEGRETURN (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4GetattrResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `attrs = ${(0, exports.formatNfsv4Bitmap)(res.resok.objAttributes.attrmask)}`);
        }
        return `GETATTR (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4GetfhResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `GETFH (${(0, exports.formatNfsv4Stat)(res.status)}) fh = ${formatFileHandle(res.resok.object)}`;
        }
        return `GETFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LinkResponse) {
        return `LINK (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LockResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `LOCK (${(0, exports.formatNfsv4Stat)(res.status)}) stateid = ${formatStateid(res.resok.lockStateid, tab)}`;
        }
        return `LOCK (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LocktResponse) {
        return `LOCKT (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LockuResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `LOCKU (${(0, exports.formatNfsv4Stat)(res.status)}) stateid = ${formatStateid(res.resok.lockStateid, tab)}`;
        }
        return `LOCKU (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LookupResponse) {
        return `LOOKUP (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4LookuppResponse) {
        return `LOOKUPP (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4NverifyResponse) {
        return `NVERIFY (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4OpenResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `OPEN (${(0, exports.formatNfsv4Stat)(res.status)}) stateid = ${formatStateid(res.resok.stateid, tab)}`;
        }
        return `OPEN (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4OpenattrResponse) {
        return `OPENATTR (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4OpenConfirmResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `stateid = ${formatStateid(res.resok.openStateid, tab)}`);
        }
        return `OPEN_CONFIRM (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4OpenDowngradeResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `stateid = ${formatStateid(res.resok.openStateid, tab)}`);
        }
        return `OPEN_DOWNGRADE (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4PutfhResponse) {
        return `PUTFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4PutpubfhResponse) {
        return `PUTPUBFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4PutrootfhResponse) {
        return `PUTROOTFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4ReadResponse) {
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            return `READ (${(0, exports.formatNfsv4Stat)(res.status)}) eof = ${res.resok.eof}, length = ${res.resok.data.length}`;
        }
        return `READ (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4ReaddirResponse) {
        return `READDIR (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4ReadlinkResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `link = "${res.resok.link}"`);
        }
        return `READLINK (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4RemoveResponse) {
        return `REMOVE (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4RenameResponse) {
        return `RENAME (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4RenewResponse) {
        return `RENEW (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4RestorefhResponse) {
        return `RESTOREFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4SavefhResponse) {
        return `SAVEFH (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4SecinfoResponse) {
        return `SECINFO (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4SetattrResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `attrsset = ${(0, exports.formatNfsv4Bitmap)(res.resok.attrsset)}`);
        }
        return `SETATTR (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4SetclientidResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `clientid = ${res.resok.clientid}`);
        }
        return `SETCLIENTID (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4SetclientidConfirmResponse) {
        return `SETCLIENTID_CONFIRM (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4VerifyResponse) {
        return `VERIFY (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4WriteResponse) {
        const items = [];
        if (res.status === 0 /* constants.Nfsv4Stat.NFS4_OK */ && res.resok) {
            items.push((tab) => `count = ${res.resok.count}`);
            items.push((tab) => `committed = ${(0, exports.formatNfsv4StableHow)(res.resok.committed)}`);
        }
        return `WRITE (${(0, exports.formatNfsv4Stat)(res.status)})` + (0, printTree_1.printTree)(tab, items);
    }
    else if (res instanceof msg.Nfsv4ReleaseLockOwnerResponse) {
        return `RELEASE_LOCKOWNER (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    else if (res instanceof msg.Nfsv4IllegalResponse) {
        return `ILLEGAL (${(0, exports.formatNfsv4Stat)(res.status)})`;
    }
    return 'Unknown Response';
};
exports.formatNfsv4Response = formatNfsv4Response;
const formatNfsv4CompoundRequest = (req, tab = '') => {
    const items = [
        (tab) => `tag = "${req.tag}"`,
        (tab) => `minorversion = ${req.minorversion}`,
    ];
    req.argarray.forEach((op, i) => {
        items.push((tab) => `[${i}] ${(0, exports.formatNfsv4Request)(op, tab)}`);
    });
    return 'COMPOUND' + (0, printTree_1.printTree)(tab, items);
};
exports.formatNfsv4CompoundRequest = formatNfsv4CompoundRequest;
const formatNfsv4CompoundResponse = (res, tab = '') => {
    const items = [
        (tab) => `status = ${(0, exports.formatNfsv4Stat)(res.status)}`,
        (tab) => `tag = "${res.tag}"`,
    ];
    res.resarray.forEach((op, i) => {
        items.push((tab) => `[${i}] ${(0, exports.formatNfsv4Response)(op, tab)}`);
    });
    return 'COMPOUND' + (0, printTree_1.printTree)(tab, items);
};
exports.formatNfsv4CompoundResponse = formatNfsv4CompoundResponse;
//# sourceMappingURL=format.js.map