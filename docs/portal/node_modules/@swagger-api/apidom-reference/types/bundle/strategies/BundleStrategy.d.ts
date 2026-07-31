import { ParseResultElement } from '@swagger-api/apidom-core';
import File from '../../File.ts';
import type { ReferenceOptions } from '../../options/index.ts';
/**
 * @public
 */
export interface BundleStrategyOptions {
    readonly name: string;
}
/**
 * @public
 */
declare abstract class BundleStrategy {
    readonly name: string;
    constructor({ name }: BundleStrategyOptions);
    abstract canBundle(file: File, options: ReferenceOptions): boolean;
    abstract bundle(file: File, options: ReferenceOptions): Promise<ParseResultElement>;
}
export default BundleStrategy;
