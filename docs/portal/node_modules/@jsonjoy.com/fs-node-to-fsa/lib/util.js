"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.newNoModificationAllowedError = exports.newNotAllowedError = exports.newTypeMismatchError = exports.newNotFoundError = exports.assertCanWrite = exports.assertName = exports.ctx = exports.basename = void 0;
const fs_fsa_1 = require("@jsonjoy.com/fs-fsa");
var fs_node_utils_1 = require("@jsonjoy.com/fs-node-utils");
Object.defineProperty(exports, "basename", { enumerable: true, get: function () { return fs_node_utils_1.basename; } });
/**
 * Creates a new {@link NodeFsaContext}.
 */
const ctx = (partial = {}) => {
    return {
        separator: '/',
        syncHandleAllowed: false,
        mode: 'read',
        locks: new fs_fsa_1.FileLockManager(),
        ...partial,
    };
};
exports.ctx = ctx;
const nameRegex = /^(\.{1,2})$|^(.*([\/\\]).*)$/;
const assertName = (name, method, klass) => {
    const isInvalid = !name || nameRegex.test(name);
    if (isInvalid)
        throw new TypeError(`Failed to execute '${method}' on '${klass}': Name is not allowed.`);
};
exports.assertName = assertName;
const assertCanWrite = (mode) => {
    if (mode !== 'readwrite')
        throw new DOMException('The request is not allowed by the user agent or the platform in the current context.', 'NotAllowedError');
};
exports.assertCanWrite = assertCanWrite;
const newNotFoundError = () => new DOMException('A requested file or directory could not be found at the time an operation was processed.', 'NotFoundError');
exports.newNotFoundError = newNotFoundError;
const newTypeMismatchError = () => new DOMException('The path supplied exists, but was not an entry of requested type.', 'TypeMismatchError');
exports.newTypeMismatchError = newTypeMismatchError;
const newNotAllowedError = () => new DOMException('Permission not granted.', 'NotAllowedError');
exports.newNotAllowedError = newNotAllowedError;
const newNoModificationAllowedError = () => new DOMException('The file is locked and cannot be modified.', 'NoModificationAllowedError');
exports.newNoModificationAllowedError = newNoModificationAllowedError;
//# sourceMappingURL=util.js.map