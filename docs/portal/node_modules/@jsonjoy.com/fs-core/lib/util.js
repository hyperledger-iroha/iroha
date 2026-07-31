"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.filenameToSteps = exports.resolve = exports.unixify = exports.isWin = void 0;
exports.isFd = isFd;
exports.validateFd = validateFd;
exports.dataToBuffer = dataToBuffer;
exports.nullCheck = nullCheck;
exports.pathToFilename = pathToFilename;
exports.createError = createError;
exports.createStatError = createStatError;
const path_1 = require("@jsonjoy.com/fs-node-builtins/lib/path");
const buffer_1 = require("@jsonjoy.com/fs-node-builtins/lib/internal/buffer");
const errors = require("@jsonjoy.com/fs-node-builtins/lib/internal/errors");
const process_1 = require("./process");
const encoding_1 = require("./encoding");
const fs_node_utils_1 = require("@jsonjoy.com/fs-node-utils");
exports.isWin = process_1.default.platform === 'win32';
const resolveCrossPlatform = path_1.resolve;
const pathSep = path_1.posix ? path_1.posix.sep : path_1.sep;
const isSeparator = (str, i) => {
    let char = str[i];
    return i > 0 && (char === '/' || (exports.isWin && char === '\\'));
};
const removeTrailingSeparator = (str) => {
    let i = str.length - 1;
    if (i < 2)
        return str;
    while (isSeparator(str, i))
        i--;
    return str.substr(0, i + 1);
};
const normalizePath = (str, stripTrailing) => {
    if (typeof str !== 'string')
        throw new TypeError('expected a string');
    str = str.replace(/[\\\/]+/g, '/');
    if (stripTrailing !== false)
        str = removeTrailingSeparator(str);
    return str;
};
const unixify = (filepath, stripTrailing = true) => {
    if (exports.isWin) {
        filepath = normalizePath(filepath, stripTrailing);
        return filepath.replace(/^([a-zA-Z]+:|\.\/)/, '');
    }
    return filepath;
};
exports.unixify = unixify;
let resolve = (filename, base = process_1.default.cwd()) => resolveCrossPlatform(base, filename);
exports.resolve = resolve;
if (exports.isWin) {
    const _resolve = resolve;
    exports.resolve = resolve = (filename, base) => (0, exports.unixify)(_resolve(filename, base));
}
const filenameToSteps = (filename, base) => {
    const fullPath = resolve(filename, base);
    const fullPathSansSlash = fullPath.substring(1);
    if (!fullPathSansSlash)
        return [];
    return fullPathSansSlash.split(pathSep);
};
exports.filenameToSteps = filenameToSteps;
function isFd(path) {
    return path >>> 0 === path;
}
function validateFd(fd) {
    if (!isFd(fd))
        throw TypeError(fs_node_utils_1.ERRSTR.FD);
}
function dataToBuffer(data, encoding = encoding_1.ENCODING_UTF8) {
    if (buffer_1.Buffer.isBuffer(data))
        return data;
    else if (data instanceof Uint8Array)
        return (0, buffer_1.bufferFrom)(data);
    else if (encoding === 'buffer')
        return (0, buffer_1.bufferFrom)(String(data), 'utf8');
    else
        return (0, buffer_1.bufferFrom)(String(data), encoding);
}
function nullCheck(path, callback) {
    if (('' + path).indexOf('\u0000') !== -1) {
        const er = new Error('Path must be a string without null bytes');
        er.code = 'ENOENT';
        if (typeof callback !== 'function')
            throw er;
        Promise.resolve().then(() => callback(er));
        return false;
    }
    return true;
}
function getPathFromURLPosix(url) {
    if (url.hostname !== '') {
        throw new errors.TypeError('ERR_INVALID_FILE_URL_HOST', process_1.default.platform);
    }
    const pathname = url.pathname;
    for (let n = 0; n < pathname.length; n++) {
        if (pathname[n] === '%') {
            const third = pathname.codePointAt(n + 2) | 0x20;
            if (pathname[n + 1] === '2' && third === 102) {
                throw new errors.TypeError('ERR_INVALID_FILE_URL_PATH', 'must not include encoded / characters');
            }
        }
    }
    return decodeURIComponent(pathname);
}
function pathToFilename(path) {
    if (path instanceof Uint8Array) {
        path = (0, buffer_1.bufferFrom)(path);
    }
    if (typeof path !== 'string' && !buffer_1.Buffer.isBuffer(path)) {
        try {
            if (!(path instanceof require('url').URL))
                throw new TypeError(fs_node_utils_1.ERRSTR.PATH_STR);
        }
        catch (err) {
            throw new TypeError(fs_node_utils_1.ERRSTR.PATH_STR);
        }
        path = getPathFromURLPosix(path);
    }
    const pathString = String(path);
    nullCheck(pathString);
    return pathString;
}
const ENOENT = 'ENOENT';
const EBADF = 'EBADF';
const EINVAL = 'EINVAL';
const EPERM = 'EPERM';
const EPROTO = 'EPROTO';
const EEXIST = 'EEXIST';
const ENOTDIR = 'ENOTDIR';
const EMFILE = 'EMFILE';
const EACCES = 'EACCES';
const EISDIR = 'EISDIR';
const ENOTEMPTY = 'ENOTEMPTY';
const ENOSYS = 'ENOSYS';
const ERR_FS_EISDIR = 'ERR_FS_EISDIR';
const ERR_OUT_OF_RANGE = 'ERR_OUT_OF_RANGE';
function formatError(errorCode, func = '', path = '', path2 = '') {
    let pathFormatted = '';
    if (path)
        pathFormatted = ` '${path}'`;
    if (path2)
        pathFormatted += ` -> '${path2}'`;
    switch (errorCode) {
        case ENOENT:
            return `ENOENT: no such file or directory, ${func}${pathFormatted}`;
        case EBADF:
            return `EBADF: bad file descriptor, ${func}${pathFormatted}`;
        case EINVAL:
            return `EINVAL: invalid argument, ${func}${pathFormatted}`;
        case EPERM:
            return `EPERM: operation not permitted, ${func}${pathFormatted}`;
        case EPROTO:
            return `EPROTO: protocol error, ${func}${pathFormatted}`;
        case EEXIST:
            return `EEXIST: file already exists, ${func}${pathFormatted}`;
        case ENOTDIR:
            return `ENOTDIR: not a directory, ${func}${pathFormatted}`;
        case EISDIR:
            return `EISDIR: illegal operation on a directory, ${func}${pathFormatted}`;
        case EACCES:
            return `EACCES: permission denied, ${func}${pathFormatted}`;
        case ENOTEMPTY:
            return `ENOTEMPTY: directory not empty, ${func}${pathFormatted}`;
        case EMFILE:
            return `EMFILE: too many open files, ${func}${pathFormatted}`;
        case ENOSYS:
            return `ENOSYS: function not implemented, ${func}${pathFormatted}`;
        case ERR_FS_EISDIR:
            return `[ERR_FS_EISDIR]: Path is a directory: ${func} returned EISDIR (is a directory) ${path}`;
        case ERR_OUT_OF_RANGE:
            return `[ERR_OUT_OF_RANGE]: value out of range, ${func}${pathFormatted}`;
        default:
            return `${errorCode}: error occurred, ${func}${pathFormatted}`;
    }
}
function createError(errorCode, func = '', path = '', path2 = '', Constructor = Error) {
    const error = new Constructor(formatError(errorCode, func, path, path2));
    error.code = errorCode;
    if (path) {
        error.path = path;
    }
    return error;
}
function createStatError(errorCode, func = '', path = '', path2 = '') {
    return {
        code: errorCode,
        message: formatError(errorCode, func, path, path2),
        path,
        toError() {
            const error = new Error(this.message);
            error.code = this.code;
            if (this.path) {
                error.path = this.path;
            }
            return error;
        },
    };
}
//# sourceMappingURL=util.js.map