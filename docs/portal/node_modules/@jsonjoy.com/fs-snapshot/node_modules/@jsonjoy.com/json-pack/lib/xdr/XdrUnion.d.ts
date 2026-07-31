/**
 * XDR Union data type that contains a discriminant and value.
 * Used for encoding XDR union types where the discriminant determines
 * which arm of the union is active.
 */
export declare class XdrUnion<T = unknown> {
    readonly discriminant: number | string | boolean;
    readonly value: T;
    constructor(discriminant: number | string | boolean, value: T);
}
