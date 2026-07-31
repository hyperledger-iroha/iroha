import { Element } from '@swagger-api/apidom-core';
/**
 * @public
 */
export declare class AncestorLineage<T extends Element> extends Array<Set<T>> {
    includesCycle(element: T): boolean;
    includes(searchElement: Set<T> | T, fromIndex?: number): boolean;
    findItem(predicate: (item: T) => boolean): T | undefined;
}
