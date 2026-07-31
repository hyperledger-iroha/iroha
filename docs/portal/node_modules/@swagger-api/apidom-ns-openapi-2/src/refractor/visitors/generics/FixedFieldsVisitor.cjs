"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
class FixedFieldsVisitor extends _SpecificationVisitor.default {
  specPath;
  ignoredFields;
  canSupportSpecificationExtensions = true;
  specificationExtensionPredicate = _predicates.isSwaggerExtension;
  constructor({
    specPath,
    ignoredFields,
    canSupportSpecificationExtensions,
    specificationExtensionPredicate,
    ...rest
  }) {
    super({
      ...rest
    });
    this.specPath = specPath;
    this.ignoredFields = ignoredFields || [];
    if (typeof canSupportSpecificationExtensions === 'boolean') {
      this.canSupportSpecificationExtensions = canSupportSpecificationExtensions;
    }
    if (typeof specificationExtensionPredicate === 'function') {
      this.specificationExtensionPredicate = specificationExtensionPredicate;
    }
  }
  ObjectElement(objectElement) {
    const specPath = this.specPath(objectElement);
    const fields = this.retrieveFixedFields(specPath);

    // @ts-ignore
    objectElement.forEach((value, key, memberElement) => {
      if ((0, _apidomCore.isStringElement)(key) && fields.includes((0, _apidomCore.toValue)(key)) && !this.ignoredFields.includes((0, _apidomCore.toValue)(key))) {
        const fixedFieldElement = this.toRefractedElement([...specPath, 'fixedFields', (0, _apidomCore.toValue)(key)], value);
        const newMemberElement = new _apidomCore.MemberElement((0, _apidomCore.cloneDeep)(key), fixedFieldElement);
        this.copyMetaAndAttributes(memberElement, newMemberElement);
        newMemberElement.classes.push('fixed-field');
        this.element.content.push(newMemberElement);
      } else if (this.canSupportSpecificationExtensions && this.specificationExtensionPredicate(memberElement)) {
        const extensionElement = this.toRefractedElement(['document', 'extension'], memberElement);
        this.element.content.push(extensionElement);
      } else if (!this.ignoredFields.includes((0, _apidomCore.toValue)(key))) {
        this.element.content.push((0, _apidomCore.cloneDeep)(memberElement));
      }
    });
    this.copyMetaAndAttributes(objectElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = FixedFieldsVisitor;