import Resolver from '../Resolver.ts';
export type { default as Resolver, ResolverOptions } from '../Resolver.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
declare class FileResolver extends Resolver {
    constructor();
    canRead(): boolean;
    read(): Promise<Buffer>;
}
export default FileResolver;
