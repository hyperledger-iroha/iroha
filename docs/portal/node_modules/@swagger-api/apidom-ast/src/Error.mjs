import Node from "./Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Error extends Node {
  static type = 'error';
  value;
  isUnexpected;
  constructor({
    value,
    isUnexpected = false,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.value = value;
    this.isUnexpected = isUnexpected;
  }
}
export default Error;