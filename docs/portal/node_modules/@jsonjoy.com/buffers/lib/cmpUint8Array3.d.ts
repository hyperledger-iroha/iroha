/**
 * Compares two `Uint8Arrays`, first by length, then by each byte. Returns a
 * negative number if `a` is less than `b`, a positive number if `a` is greater
 * than `b`, or 0 if `a` is equal to `b`.
 *
 * @returns A negative number if a is less than b, a positive number if a is
 *          greater than b, or 0 if a is equal to b.
 */
export declare const cmpUint8Array3: (a: Uint8Array, b: Uint8Array) => number;
