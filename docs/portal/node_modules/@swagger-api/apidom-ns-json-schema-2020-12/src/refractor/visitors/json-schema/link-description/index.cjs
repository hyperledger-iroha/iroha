"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2019-09");
var _LinkDescription = _interopRequireDefault(require("../../../../elements/LinkDescription.cjs"));
/**
 * @public
 */
class LinkDescriptionVisitor extends _apidomNsJsonSchema.LinkDescriptionVisitor {
  constructor(options) {
    super(options);
    this.element = new _LinkDescription.default();
  }
}
var _default = exports.default = LinkDescriptionVisitor;