"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _Responses = _interopRequireDefault(require("../../../../elements/Responses.cjs"));
var _MixedFieldsVisitor = _interopRequireDefault(require("../../generics/MixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../predicates.cjs");
var _predicates2 = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class ResponsesVisitor extends (0, _tsMixer.Mixin)(_MixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Responses.default();
    this.specPathFixedFields = (0, _ramda.always)(['document', 'objects', 'Responses']);
    this.canSupportSpecificationExtensions = true;
    this.specPathPatternedFields = element => (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Response'];
    this.fieldPatternPredicate = value => new RegExp(`^(1XX|2XX|3XX|4XX|5XX|${(0, _ramda.range)(100, 600).join('|')})$`).test(String(value));
  }
  ObjectElement(objectElement) {
    const result = _MixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(_predicates2.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'response');
    });

    // decorate every ResponseElement with metadata about their status code
    // @ts-ignore
    this.element.filter(_predicates2.isResponseElement).forEach((value, key) => {
      const httpStatusCode = (0, _apidomCore.cloneDeep)(key);
      if (!this.fieldPatternPredicate((0, _apidomCore.toValue)(httpStatusCode))) return;
      value.setMetaProperty('http-status-code', httpStatusCode);
    });
    return result;
  }
}
var _default = exports.default = ResponsesVisitor;