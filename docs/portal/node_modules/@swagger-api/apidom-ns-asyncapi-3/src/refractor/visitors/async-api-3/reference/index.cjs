"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseReferenceVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Reference = _interopRequireDefault(require("../../../../elements/Reference.cjs"));
/**
 * @public
 */
const BaseReferenceVisitor = exports.BaseReferenceVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Reference.$visitor;
/**
 * @public
 */
class ReferenceVisitor extends BaseReferenceVisitor {
  constructor(options) {
    super(options);
    this.element = new _Reference.default();
  }
}
var _default = exports.default = ReferenceVisitor;