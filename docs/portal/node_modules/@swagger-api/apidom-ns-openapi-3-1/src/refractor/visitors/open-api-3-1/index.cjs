"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _OpenApi = _interopRequireDefault(require("../../../elements/OpenApi3-1.cjs"));
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention

/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
class OpenApi3_1Visitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.FixedFieldsVisitor, _apidomNsOpenapi.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _OpenApi.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'OpenApi']);
    this.canSupportSpecificationExtensions = true;
    this.openApiSemanticElement = this.element;
  }
  ObjectElement(objectElement) {
    this.openApiGenericElement = objectElement;
    return _apidomNsOpenapi.FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
}
var _default = exports.default = OpenApi3_1Visitor;