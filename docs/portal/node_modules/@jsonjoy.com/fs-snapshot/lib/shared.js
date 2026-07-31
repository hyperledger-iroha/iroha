"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.validateEntryName = exports.writer = void 0;
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
exports.writer = new Writer_1.Writer(1024 * 32);
const validateEntryName = (name) => {
    if (!name || name === '.' || name === '..' || name.indexOf('/') !== -1 || name.indexOf('\\') !== -1)
        throw new Error(`Invalid snapshot entry name: ${JSON.stringify(name)}`);
};
exports.validateEntryName = validateEntryName;
//# sourceMappingURL=shared.js.map