"use strict";

exports.__esModule = true;
exports.YamlStyleGroup = exports.YamlStyle = void 0;
/**
 * @public
 */
let YamlStyle = exports.YamlStyle = /*#__PURE__*/function (YamlStyle) {
  YamlStyle["Plain"] = "Plain";
  YamlStyle["SingleQuoted"] = "SingleQuoted";
  YamlStyle["DoubleQuoted"] = "DoubleQuoted";
  YamlStyle["Literal"] = "Literal";
  YamlStyle["Folded"] = "Folded";
  YamlStyle["Explicit"] = "Explicit";
  YamlStyle["SinglePair"] = "SinglePair";
  YamlStyle["NextLine"] = "NextLine";
  YamlStyle["InLine"] = "InLine";
  return YamlStyle;
}({});
/**
 * @public
 */
let YamlStyleGroup = exports.YamlStyleGroup = /*#__PURE__*/function (YamlStyleGroup) {
  YamlStyleGroup["Flow"] = "Flow";
  YamlStyleGroup["Block"] = "Block";
  return YamlStyleGroup;
}({});