"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class PatternedFieldsVisitor extends _SpecificationVisitor.default {
  specPath;
  ignoredFields;
  fieldPatternPredicate = _ramda.F;
  canSupportSpecificationExtensions = false;
  specificationExtensionPredicate = _predicates.isAsyncApiExtension;
  constructor({
    specPath,
    ignoredFields,
    fieldPatternPredicate,
    canSupportSpecificationExtensions,
    specificationExtensionPredicate,
    ...rest
  }) {
    super({
      ...rest
    });
    this.specPath = specPath;
    this.ignoredFields = ignoredFields || [];
    if (typeof fieldPatternPredicate === 'function') {
      this.fieldPatternPredicate = fieldPatternPredicate;
    }
    if (typeof canSupportSpecificationExtensions === 'boolean') {
      this.canSupportSpecificationExtensions = canSupportSpecificationExtensions;
    }
    if (typeof specificationExtensionPredicate === 'function') {
      this.specificationExtensionPredicate = specificationExtensionPredicate;
    }
  }
  ObjectElement(objectElement) {
    // @ts-ignore
    objectElement.forEach((value, key, memberElement) => {
      if (this.canSupportSpecificationExtensions && this.specificationExtensionPredicate(memberElement)) {
        const extensionElement = this.toRefractedElement(['document', 'extension'], memberElement);
        this.element.content.push(extensionElement);
      } else if (!this.ignoredFields.includes((0, _apidomCore.toValue)(key)) && this.fieldPatternPredicate((0, _apidomCore.toValue)(key))) {
        const specPath = this.specPath(value);
        const patternedFieldElement = this.toRefractedElement(specPath, value);
        const newMemberElement = new _apidomCore.MemberElement((0, _apidomCore.cloneDeep)(key), patternedFieldElement);
        this.copyMetaAndAttributes(memberElement, newMemberElement);
        newMemberElement.classes.push('patterned-field');
        this.element.content.push(newMemberElement);
      } else if (!this.ignoredFields.includes((0, _apidomCore.toValue)(key))) {
        this.element.content.push((0, _apidomCore.cloneDeep)(memberElement));
      }
    });
    this.copyMetaAndAttributes(objectElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = PatternedFieldsVisitor;