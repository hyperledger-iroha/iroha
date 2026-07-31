import { Element } from '@swagger-api/apidom-core';
import EvaluationJsonSchemaUriError from '../../../../errors/EvaluationJsonSchemaUriError.ts';
/**
 * Evaluates JSON Schema $ref containing unknown URI against ApiDOM fragment.
 * @public
 */
export declare const evaluate: {
    <T extends Element>(uri: string, element: T): Element | undefined;
    cache: WeakMap<WeakKey, any>;
};
export { EvaluationJsonSchemaUriError };
export { default as JsonSchemaUriError } from '../../../../errors/JsonSchemaUriError.ts';
