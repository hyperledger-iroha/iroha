import File from '../../../File.ts';
import Resolver, { ResolverOptions } from '../Resolver.ts';
export type { default as Resolver, ResolverOptions } from '../Resolver.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface FileResolverOptions extends Omit<ResolverOptions, 'name'> {
    readonly fileAllowList?: string[] | RegExp[];
}
/**
 * @public
 */
declare class FileResolver extends Resolver {
    fileAllowList: string[] | RegExp[];
    constructor(options?: FileResolverOptions);
    canRead(file: File): boolean;
    read(file: File): Promise<Buffer>;
}
export default FileResolver;
