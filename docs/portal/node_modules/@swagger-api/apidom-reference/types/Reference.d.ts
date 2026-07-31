import { Element } from '@swagger-api/apidom-core';
import ReferenceSet from './ReferenceSet.ts';
/**
 * @public
 */
export interface ReferenceOptions<T = Element> {
    readonly uri: string;
    readonly depth?: number;
    readonly refSet?: ReferenceSet;
    readonly value: T;
}
/**
 * @public
 */
declare class Reference<T = Element> {
    readonly uri: string;
    readonly depth: number;
    readonly value: T;
    refSet?: ReferenceSet;
    readonly errors: Array<Error>;
    constructor({ uri, depth, refSet, value }: ReferenceOptions<T>);
}
export default Reference;
