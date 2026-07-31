"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.cloneBinary = void 0;
const isUint8Array_1 = require("@jsonjoy.com/buffers/lib/isUint8Array");
const { isArray } = Array;
const objectKeys = Object.keys;
/**
 * Creates a deep clone of any JSON-like object.
 *
 * @param obj Any plain POJO object.
 * @returns A deep copy of the object.
 */
const cloneBinary = (obj) => {
    if (!obj)
        return obj;
    if (isArray(obj)) {
        const arr = [];
        const length = obj.length;
        for (let i = 0; i < length; i++)
            arr.push((0, exports.cloneBinary)(obj[i]));
        return arr;
    }
    else if (typeof obj === 'object') {
        if ((0, isUint8Array_1.isUint8Array)(obj))
            return new Uint8Array(obj);
        const keys = objectKeys(obj);
        const length = keys.length;
        const newObject = {};
        for (let i = 0; i < length; i++) {
            const key = keys[i];
            newObject[key] = (0, exports.cloneBinary)(obj[key]);
        }
        return newObject;
    }
    return obj;
};
exports.cloneBinary = cloneBinary;
//# sourceMappingURL=cloneBinary.js.map