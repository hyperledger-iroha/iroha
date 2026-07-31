"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlScalar = _interopRequireDefault(require("../nodes/YamlScalar.cjs"));
var _YamlReferenceError = _interopRequireDefault(require("../errors/YamlReferenceError.cjs"));
var _predicates = require("../nodes/predicates.cjs");
var _YamlStyle = require("../nodes/YamlStyle.cjs");
/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class ReferenceManager {
  addAnchor(node) {
    if (!(0, _predicates.isAnchor)(node.anchor)) {
      throw new _YamlReferenceError.default('Expected YAML anchor to be attached the the YAML AST node.', {
        node
      });
    }
  }
  resolveAlias(alias) {
    return new _YamlScalar.default({
      content: alias.content,
      style: _YamlStyle.YamlStyle.Plain,
      styleGroup: _YamlStyle.YamlStyleGroup.Flow
    });
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = ReferenceManager;