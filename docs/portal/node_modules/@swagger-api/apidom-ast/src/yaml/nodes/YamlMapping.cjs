"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlCollection = _interopRequireDefault(require("./YamlCollection.cjs"));
var _predicates = require("./predicates.cjs");
/**
 * @public
 */
class YamlMapping extends _YamlCollection.default {
  static type = 'mapping';
}
Object.defineProperty(YamlMapping.prototype, 'content', {
  get() {
    return Array.isArray(this.children) ? this.children.filter(_predicates.isKeyValuePair) : [];
  },
  enumerable: true
});
var _default = exports.default = YamlMapping;