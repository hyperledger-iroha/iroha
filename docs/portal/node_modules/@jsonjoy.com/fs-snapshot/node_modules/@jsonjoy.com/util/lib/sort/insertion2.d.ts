/**
 * Insertion sort, should be faster than built-int sort for small arrays.
 *
 * @todo Move this to `thingies` package.
 *
 * @param arr Array to sort.
 * @param comparator Comparator function.
 * @returns Returns the same array instance.
 */
export declare const sort: <T>(arr: T[], comparator: (a: T, b: T) => number) => T[];
