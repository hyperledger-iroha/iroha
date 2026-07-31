"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseParametersVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _Parameters = _interopRequireDefault(require("../../../../elements/Parameters.cjs"));
const BaseParametersVisitor = exports.BaseParametersVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.Parameters.$visitor;
/**
 * @public
 */
class ParametersVisitor extends BaseParametersVisitor {
  constructor(options) {
    super(options);
    this.element = new _Parameters.default();
  }
}
var _default = exports.default = ParametersVisitor;