import Node from "../../Node.mjs";
/**
 * @public
 */
export let YamlNodeKind = /*#__PURE__*/function (YamlNodeKind) {
  YamlNodeKind["Scalar"] = "Scalar";
  YamlNodeKind["Sequence"] = "Sequence";
  YamlNodeKind["Mapping"] = "Mapping";
  return YamlNodeKind;
}({});

/**
 * @public
 */

/**
 * @public
 */
class YamlTag extends Node {
  static type = 'tag';
  explicitName;
  kind;
  constructor({
    explicitName,
    kind,
    ...rest
  }) {
    super({
      ...rest
    });
    this.explicitName = explicitName;
    this.kind = kind;
  }
}
export default YamlTag;