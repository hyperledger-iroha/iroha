"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.normalizeNodeFsError = exports.isErrCode = void 0;
const isErrCode = (code, error) => !!error && typeof error === 'object' && error.code === code;
exports.isErrCode = isErrCode;
const normalizeNodeFsError = (err, logger) => {
    if ((0, exports.isErrCode)('ENOENT', err))
        return 2 /* Nfsv4Stat.NFS4ERR_NOENT */;
    if ((0, exports.isErrCode)('EACCES', err))
        return 13 /* Nfsv4Stat.NFS4ERR_ACCESS */;
    if ((0, exports.isErrCode)('EEXIST', err))
        return 17 /* Nfsv4Stat.NFS4ERR_EXIST */;
    logger.error('UNEXPECTED_FS_ERROR', err);
    return 5 /* Nfsv4Stat.NFS4ERR_IO */;
};
exports.normalizeNodeFsError = normalizeNodeFsError;
//# sourceMappingURL=util.js.map