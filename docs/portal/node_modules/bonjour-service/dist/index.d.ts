import BonjourService, { Service as ServiceClass, Browser as BrowserClass } from './lib/bonjour';
import type { ServiceReferer as ServiceRefererType, ServiceConfig as ServiceConfigType, BrowserConfig as BrowserConfigType } from './lib/bonjour';
declare class Bonjour extends BonjourService {
}
declare namespace Bonjour {
    const Bonjour: typeof import(".");
    const Service: typeof ServiceClass;
    const Browser: typeof BrowserClass;
    type ServiceReferer = ServiceRefererType;
    type ServiceConfig = ServiceConfigType;
    type BrowserConfig = BrowserConfigType;
}
export = Bonjour;
