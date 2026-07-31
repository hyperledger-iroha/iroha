"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.cmpUint8Array3 = void 0;
/**
 * Compares two `Uint8Arrays`, first by length, then by each byte. Returns a
 * negative number if `a` is less than `b`, a positive number if `a` is greater
 * than `b`, or 0 if `a` is equal to `b`.
 *
 * @returns A negative number if a is less than b, a positive number if a is
 *          greater than b, or 0 if a is equal to b.
 */
const cmpUint8Array3 = (a, b) => {
    const len1 = a.length;
    const len2 = b.length;
    const diff = len1 - len2;
    if (diff !== 0)
        return diff;
    for (let i = 0; i < len1; i++) {
        const diff = a[i] - b[i];
        if (diff !== 0)
            return diff;
    }
    return 0;
};
exports.cmpUint8Array3 = cmpUint8Array3;
//# sourceMappingURL=cmpUint8Array3.js.map