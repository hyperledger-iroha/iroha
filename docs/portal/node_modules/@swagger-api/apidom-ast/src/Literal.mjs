import Node from "./Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Literal extends Node {
  static type = 'literal';
  value;
  constructor({
    value,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.value = value;
  }
}
export default Literal;