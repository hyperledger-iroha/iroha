import { ApiDOMError } from '@swagger-api/apidom-error';
/**
 * @public
 */
declare class PluginError extends ApiDOMError {
    plugin: any;
    constructor(message: string, options: {
        cause?: Error;
        plugin: any;
    });
}
export default PluginError;
