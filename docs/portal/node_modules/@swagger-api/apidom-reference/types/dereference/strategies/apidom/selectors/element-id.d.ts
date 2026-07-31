import { Element } from '@swagger-api/apidom-core';
import EvaluationElementIdError from '../../../../errors/EvaluationElementIdError.ts';
/**
 * Evaluates element ID against ApiDOM fragment.
 * @public
 */
export declare const evaluate: {
    <T extends Element>(elementID: string, element: T): Element | undefined;
    cache: WeakMap<WeakKey, any>;
};
export { EvaluationElementIdError };
