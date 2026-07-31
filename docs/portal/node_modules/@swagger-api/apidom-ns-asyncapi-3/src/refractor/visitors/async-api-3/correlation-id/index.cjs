"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseCorrelationIDVisitor = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _CorrelationID = _interopRequireDefault(require("../../../../elements/CorrelationID.cjs"));
const BaseCorrelationIDVisitor = exports.BaseCorrelationIDVisitor = _apidomNsAsyncapi.specificationObj.visitors.document.objects.CorrelationID.$visitor;
/**
 * @public
 */
class CorrelationIDVisitor extends BaseCorrelationIDVisitor {
  constructor(options) {
    super(options);
    this.element = new _CorrelationID.default();
  }
}
var _default = exports.default = CorrelationIDVisitor;