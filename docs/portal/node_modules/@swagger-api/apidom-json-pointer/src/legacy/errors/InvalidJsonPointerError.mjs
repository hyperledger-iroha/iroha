import JsonPointerError from "./JsonPointerError.mjs";
/**
 * @public
 * @deprecated
 */
/**
 * @public
 * @deprecated
 */
class InvalidJsonPointerError extends JsonPointerError {
  pointer;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.pointer = structuredOptions.pointer;
    }
  }
}
export default InvalidJsonPointerError;