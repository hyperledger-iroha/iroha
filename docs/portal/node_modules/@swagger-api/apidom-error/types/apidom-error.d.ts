/**
 * @public
 */
export declare class ApiDOMAggregateError extends AggregateError {
    constructor(errors: Iterable<unknown>, message?: string, options?: ApiDOMErrorOptions);
}

/**
 * @public
 */
export declare class ApiDOMError extends Error {
    static [Symbol.hasInstance](instance: unknown): boolean;
    constructor(message?: string, options?: ApiDOMErrorOptions);
}

/**
 * @public
 */
export declare interface ApiDOMErrorOptions extends ErrorOptions {
    readonly cause?: unknown;
    readonly [key: string]: unknown;
}

/**
 * @public
 */
export declare class ApiDOMStructuredError extends ApiDOMError {
    constructor(message?: string, structuredOptions?: ApiDOMErrorOptions);
}

/**
 * @public
 */
export declare class NotImplementedError extends UnsupportedOperationError {
}

/**
 * @public
 */
export declare class UnsupportedOperationError extends ApiDOMError {
}

export { }
