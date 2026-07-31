/**
 * Wraps a function into a proxy function with the same signature, but which can
 * be re-implemented by the user at runtime.
 *
 * @param implementation Initial implementation.
 * @returns Proxy function and implementation setter.
 */
export declare const dynamicFunction: <F extends (...args: any[]) => any>(implementation: F) => [fn: F, set: (fn: F) => void];
