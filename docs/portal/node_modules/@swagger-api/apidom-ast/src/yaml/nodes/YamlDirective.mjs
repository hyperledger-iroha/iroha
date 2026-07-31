import { mergeRight } from 'ramda';
import Node from "../../Node.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
class YamlDirective extends Node {
  static type = 'directive';
  name;
  parameters;
  constructor({
    name,
    parameters,
    ...rest
  }) {
    super({
      ...rest
    });
    this.name = name;
    this.parameters = mergeRight({
      version: undefined,
      handle: undefined,
      prefix: undefined
    }, parameters);
  }
}
export default YamlDirective;