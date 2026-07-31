"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _ramda = require("ramda");
var _Operation = _interopRequireDefault(require("../../../../elements/Operation.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */
class OperationVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Operation.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Operation']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    const action = objectElement.get('action');
    if ((0, _apidomCore.isStringElement)(action)) {
      const actionValue = action.toValue();
      if (actionValue === 'send' || actionValue === 'receive') {
        this.element.setMetaProperty('operation-action', actionValue);
      }
    }
    return result;
  }
}
var _default = exports.default = OperationVisitor;