import type Reference from './Reference.ts';
/**
 * @public
 */
export interface ReferenceSetOptions {
    readonly refs?: Reference[];
    readonly circular?: boolean;
}
/**
 * @public
 */
declare class ReferenceSet {
    rootRef?: Reference;
    readonly refs: Reference[];
    circular: boolean;
    constructor({ refs, circular }?: ReferenceSetOptions);
    get size(): number;
    add(reference: Reference): this;
    merge(anotherRefSet: this): this;
    has(thing: string | Reference): boolean;
    find(predicate: (value: Reference, index: number, obj: Reference[]) => boolean): Reference | undefined;
    values(): Generator<Reference<import("minim").Element>, void, undefined>;
    clean(): void;
}
export default ReferenceSet;
