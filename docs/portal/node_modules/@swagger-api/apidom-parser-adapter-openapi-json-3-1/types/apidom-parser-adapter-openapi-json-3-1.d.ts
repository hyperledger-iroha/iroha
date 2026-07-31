import { Namespace } from '@swagger-api/apidom-core';
import { OpenAPIMediaTypes } from '@swagger-api/apidom-ns-openapi-3-1';
import { ParseResultElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
export declare const detect: (source: string) => Promise<boolean>;

/**
 * @public
 */
export declare const detectionRegExp: RegExp;

/**
 * @public
 */
export declare const mediaTypes: OpenAPIMediaTypes;

/**
 * @public
 */
export declare const namespace: Namespace;

/**
 * @public
 */
export declare const parse: (source: string, options?: Record<string, unknown>) => Promise<ParseResultElement>;

export { }
