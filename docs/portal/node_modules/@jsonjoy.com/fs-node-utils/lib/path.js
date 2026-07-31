"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.basename = void 0;
const basename = (path, separator) => {
    if (path[path.length - 1] === separator)
        path = path.slice(0, -1);
    const lastSlashIndex = path.lastIndexOf(separator);
    return lastSlashIndex === -1 ? path : path.slice(lastSlashIndex + 1);
};
exports.basename = basename;
//# sourceMappingURL=path.js.map