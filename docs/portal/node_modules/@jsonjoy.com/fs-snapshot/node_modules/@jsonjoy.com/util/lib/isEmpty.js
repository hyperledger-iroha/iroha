"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.isEmpty = void 0;
const isEmpty = (obj) => {
    for (const key in obj)
        if (
        // biome-ignore lint: .hasOwnProperty access is intentional
        Object.prototype.hasOwnProperty.call(obj, key))
            return false;
    return true;
};
exports.isEmpty = isEmpty;
//# sourceMappingURL=isEmpty.js.map