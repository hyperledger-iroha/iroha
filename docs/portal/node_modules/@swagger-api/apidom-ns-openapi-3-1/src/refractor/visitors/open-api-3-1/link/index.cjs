"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseLinkVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Link = _interopRequireDefault(require("../../../../elements/Link.cjs"));
/**
 * @public
 */
const BaseLinkVisitor = exports.BaseLinkVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Link.$visitor;
/**
 * @public
 */
class LinkVisitor extends BaseLinkVisitor {
  constructor(options) {
    super(options);
    this.element = new _Link.default();
  }
}
var _default = exports.default = LinkVisitor;