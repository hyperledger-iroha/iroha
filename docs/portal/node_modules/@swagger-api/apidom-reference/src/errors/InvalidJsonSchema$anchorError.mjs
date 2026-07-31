import JsonSchema$anchorError from "./JsonSchema$anchorError.mjs";
/**
 * @public
 */
class InvalidJsonSchema$anchorError extends JsonSchema$anchorError {
  constructor(anchor) {
    super(`Invalid JSON Schema $anchor "${anchor}".`);
  }
}
export default InvalidJsonSchema$anchorError;