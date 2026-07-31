/**
 * Computes exact prices JSON size as would be output from JSON.stringify().
 *
 * @param value JSON value to approximate size of
 * @returns Size in bytes of JSON value
 */
export declare const jsonSize: (value: unknown) => number;
/**
 * Same as `jsonSize` function, but approximates the size of strings to improve performance.
 * Uses `.length` property of strings to approximate their size.
 *
 * @param value JSON value to approximate size of
 * @returns Size in bytes of JSON value
 */
export declare const jsonSizeApprox: (value: unknown) => number;
