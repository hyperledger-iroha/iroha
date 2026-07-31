"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.hasOwnProperty = hasOwnProperty;
const has = Object.prototype.hasOwnProperty;
// biome-ignore lint: shadow name is intended
function hasOwnProperty(obj, key) {
    return has.call(obj, key);
}
//# sourceMappingURL=hasOwnProperty.js.map