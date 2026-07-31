import { Element } from 'minim';
import File from '../../File.ts';
import type { ReferenceOptions } from '../../options/index.ts';
/**
 * @public
 */
export interface DereferenceStrategyOptions {
    readonly name: string;
}
/**
 * @public
 */
declare abstract class DereferenceStrategy {
    readonly name: string;
    constructor({ name }: DereferenceStrategyOptions);
    abstract canDereference(file: File, options: ReferenceOptions): boolean;
    abstract dereference(file: File, options: ReferenceOptions): Promise<Element>;
}
export default DereferenceStrategy;
