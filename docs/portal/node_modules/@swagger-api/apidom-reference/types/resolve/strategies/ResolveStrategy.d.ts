import File from '../../File.ts';
import ReferenceSet from '../../ReferenceSet.ts';
import type { ReferenceOptions } from '../../options/index.ts';
/**
 * @public
 */
export interface ResolveStrategyOptions {
    readonly name: string;
}
/**
 * @public
 */
declare abstract class ResolveStrategy {
    readonly name: string;
    constructor({ name }: ResolveStrategyOptions);
    abstract canResolve(file: File, options: ReferenceOptions): boolean;
    abstract resolve(file: File, options: ReferenceOptions): Promise<ReferenceSet>;
}
export default ResolveStrategy;
