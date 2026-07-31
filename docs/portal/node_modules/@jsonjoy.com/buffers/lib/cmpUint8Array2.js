"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.cmpUint8Array2 = void 0;
/**
 * Compares two `Uint8Arrays` byte-by-byte. Returns a negative number if `a` is
 * less than `b`, a positive number if `a` is greater than `b`, or 0 if `a` is
 * equal to `b`.
 *
 * @returns A negative number if a is less than b, a positive number if a is
 *         greater than b, or 0 if a is equal to b.
 */
const cmpUint8Array2 = (a, b) => {
    const len1 = a.length;
    const len2 = b.length;
    const len = Math.min(len1, len2);
    for (let i = 0; i < len; i++) {
        const diffChar = a[i] - b[i];
        if (diffChar !== 0)
            return diffChar;
    }
    return len1 - len2;
};
exports.cmpUint8Array2 = cmpUint8Array2;
//# sourceMappingURL=cmpUint8Array2.js.map