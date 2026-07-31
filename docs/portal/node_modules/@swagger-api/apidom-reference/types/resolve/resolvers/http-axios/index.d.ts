import { Axios, AxiosInstance, CreateAxiosDefaults } from 'axios';
import HTTPResolver, { HTTPResolverOptions } from '../HTTPResolver.ts';
import File from '../../../File.ts';
export type { default as HTTPResolver, HTTPResolverOptions } from '../HTTPResolver.ts';
export type { default as Resolver, ResolverOptions } from '../Resolver.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface HTTPResolverAxiosConfig extends CreateAxiosDefaults {
    interceptors?: Axios['interceptors'];
}
/**
 * @public
 */
export interface HTTPResolverAxiosOptions extends Omit<HTTPResolverOptions, 'name'> {
    readonly axiosConfig?: HTTPResolverAxiosConfig;
}
/**
 * @public
 */
declare class HTTPResolverAxios extends HTTPResolver {
    axiosConfig: HTTPResolverAxiosConfig;
    protected axiosInstance: AxiosInstance;
    protected previousAxiosConfig: HTTPResolverAxiosConfig;
    constructor(options?: HTTPResolverAxiosOptions);
    getHttpClient(): AxiosInstance;
    read(file: File): Promise<Buffer>;
}
export default HTTPResolverAxios;
