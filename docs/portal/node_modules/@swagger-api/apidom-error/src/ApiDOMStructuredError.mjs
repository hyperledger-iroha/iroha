import ApiDOMError from "./ApiDOMError.mjs";
/**
 * @public
 */
class ApiDOMStructuredError extends ApiDOMError {
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (structuredOptions != null && typeof structuredOptions === 'object') {
      const {
        cause,
        ...causelessOptions
      } = structuredOptions;
      Object.assign(this, causelessOptions);
    }
  }
}
export default ApiDOMStructuredError;