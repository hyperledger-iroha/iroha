/**
 * Filters the given plugins, returning only the ones return `true` for the given method.
 */
export declare const filter: (method: string, parameters: any[], plugins: Array<any>) => Promise<Array<any>>;
/**
 * Runs the specified method of the given plugins, in order,
 * until one of them returns a successful result.
 * Each method can return a synchronous value or a Promise.
 * If the promise resolves successfully then the result
 * is immediately returned and no further plugins are called.
 * If the promise rejects then the next plugin is called.
 * If ALL plugins fail, then the last error is thrown.
 */
export declare const run: (method: string, parameters: any[], plugins: any[]) => Promise<any>;
