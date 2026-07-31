import Browser, { BrowserConfig } from './browser';
import Service, { ServiceConfig, ServiceReferer } from './service';
export default class Bonjour {
    private server;
    private registry;
    constructor(opts?: Partial<ServiceConfig>, errorCallback?: Function | undefined);
    publish(opts: ServiceConfig): Service;
    unpublishAll(callback?: CallableFunction | undefined): void;
    find(opts?: BrowserConfig | null, onup?: (service: Service) => void): Browser;
    findOne(opts?: BrowserConfig | null, timeout?: number, callback?: CallableFunction): Browser;
    destroy(callback?: CallableFunction): void;
}
export { Service, ServiceReferer, ServiceConfig, Browser, BrowserConfig };
