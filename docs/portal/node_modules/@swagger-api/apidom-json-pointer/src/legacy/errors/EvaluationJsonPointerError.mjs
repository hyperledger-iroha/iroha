import JsonPointerError from "./JsonPointerError.mjs";
/**
 * @public
 * @deprecated
 */
/**
 * @public
 * @deprecated
 */
class EvaluationJsonPointerError extends JsonPointerError {
  pointer;
  tokens;
  failedToken;
  failedTokenPosition;
  element;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.pointer = structuredOptions.pointer;
      if (Array.isArray(structuredOptions.tokens)) {
        this.tokens = [...structuredOptions.tokens];
      }
      this.failedToken = structuredOptions.failedToken;
      this.failedTokenPosition = structuredOptions.failedTokenPosition;
      this.element = structuredOptions.element;
    }
  }
}
export default EvaluationJsonPointerError;