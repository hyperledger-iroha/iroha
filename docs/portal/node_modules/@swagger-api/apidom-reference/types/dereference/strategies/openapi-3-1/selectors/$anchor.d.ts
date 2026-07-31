import { Element } from '@swagger-api/apidom-core';
import EvaluationJsonSchema$anchorError from '../../../../errors/EvaluationJsonSchema$anchorError.ts';
import InvalidJsonSchema$anchorError from '../../../../errors/InvalidJsonSchema$anchorError.ts';
/**
 * @public
 */
export declare const isAnchor: (uri: string) => boolean;
/**
 * @public
 */
export declare const uriToAnchor: (uri: string) => string;
/**
 * @public
 */
export declare const parse: (anchor: string) => string;
/**
 * Evaluates JSON Schema $anchor against ApiDOM fragment.
 * @public
 */
export declare const evaluate: <T extends Element>(anchor: string, element: T) => Element | undefined;
export { EvaluationJsonSchema$anchorError, InvalidJsonSchema$anchorError };
export { default as JsonSchema$anchorError } from '../../../../errors/JsonSchema$anchorError.ts';
