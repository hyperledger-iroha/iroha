import KeyValue from './KeyValue';
import { EventEmitter } from 'events';
import Service from './service';
export interface BrowserConfig {
    type: string;
    name?: string;
    protocol?: 'tcp' | 'udp';
    subtypes?: string[];
    txt?: KeyValue;
}
export type BrowserOnUp = (service: Service) => void;
export interface BrowserEvents {
    up: [service: Service];
    down: [service: Service];
    'txt-update': [newService: Service, existingService: Service];
    'srv-update': [newService: Service, existingService: Service];
}
export declare class Browser extends EventEmitter<BrowserEvents> {
    private mdns;
    private onresponse;
    private serviceMap;
    private txt;
    private name?;
    private txtQuery;
    private wildcard;
    private _services;
    constructor(mdns: any, opts: BrowserConfig | BrowserOnUp | null, onup?: BrowserOnUp);
    start(): void;
    stop(): void;
    update(): void;
    expire(): void;
    get services(): Service[];
    private addService;
    private updateServiceSrv;
    private updateServiceTxt;
    private replaceService;
    private removeService;
    private goodbyes;
    private buildServicesFor;
}
export default Browser;
