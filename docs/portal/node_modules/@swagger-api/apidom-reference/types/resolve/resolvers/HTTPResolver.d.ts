import Resolver, { ResolverOptions } from './Resolver.ts';
import File from '../../File.ts';
/**
 * @public
 */
export interface HTTPResolverOptions extends ResolverOptions {
    readonly timeout?: number;
    readonly redirects?: number;
    readonly withCredentials?: boolean;
}
/**
 * @public
 */
declare abstract class HTTPResolver extends Resolver {
    protected readonly timeout: number;
    protected readonly redirects: number;
    protected readonly withCredentials: boolean;
    constructor(options?: HTTPResolverOptions);
    canRead(file: File): boolean;
    abstract getHttpClient(): unknown;
}
export default HTTPResolver;
