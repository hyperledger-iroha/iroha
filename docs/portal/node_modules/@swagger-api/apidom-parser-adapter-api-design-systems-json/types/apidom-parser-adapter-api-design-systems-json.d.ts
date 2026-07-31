import { ApiDesignSystemsMediaTypes } from '@swagger-api/apidom-ns-api-design-systems';
import { Namespace } from '@swagger-api/apidom-core';
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
export declare const mediaTypes: ApiDesignSystemsMediaTypes;

/**
 * @public
 */
export declare const namespace: Namespace;

/**
 * @public
 */
export declare const parse: (source: string, options?: Record<string, unknown>) => Promise<ParseResultElement>;

export { }
