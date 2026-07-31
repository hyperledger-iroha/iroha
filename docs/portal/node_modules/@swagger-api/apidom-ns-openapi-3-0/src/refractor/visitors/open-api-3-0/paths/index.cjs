"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _Paths = _interopRequireDefault(require("../../../../elements/Paths.cjs"));
var _PatternedFieldsVisitor = _interopRequireDefault(require("../../generics/PatternedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class PathsVisitor extends (0, _tsMixer.Mixin)(_PatternedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Paths.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'PathItem']);
    this.canSupportSpecificationExtensions = true;
    this.fieldPatternPredicate = _ramda.T;
  }
  ObjectElement(objectElement) {
    const result = _PatternedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every PathItemElement with path metadata
    this.element.filter(_predicates.isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      key.classes.push('openapi-path-template');
      key.classes.push('path-template');
      pathItemElement.setMetaProperty('path', (0, _apidomCore.cloneDeep)(key));
    });
    return result;
  }
}
var _default = exports.default = PathsVisitor;