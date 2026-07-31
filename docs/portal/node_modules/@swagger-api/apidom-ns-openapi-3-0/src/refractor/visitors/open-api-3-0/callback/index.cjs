"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _Callback = _interopRequireDefault(require("../../../../elements/Callback.cjs"));
var _PatternedFieldsVisitor = _interopRequireDefault(require("../../generics/PatternedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class CallbackVisitor extends (0, _tsMixer.Mixin)(_PatternedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Callback.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'PathItem']);
    this.canSupportSpecificationExtensions = true;
    this.fieldPatternPredicate = value => /{(?<expression>[^}]{1,2083})}/.test(String(value)); // 2,083 characters is the maximum length of a URL in Chrome
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every PathItemElement with Callback Object expression metadata
    this.element.filter(_predicates.isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      pathItemElement.setMetaProperty('runtime-expression', (0, _apidomCore.toValue)(key));
    });
    return result;
  }
}
var _default = exports.default = CallbackVisitor;