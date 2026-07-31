import { F as stubFalse } from 'ramda';
import { MemberElement, BREAK, cloneDeep, toValue } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import { isOpenApiExtension } from "../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PatternedFieldsVisitor extends SpecificationVisitor {
  specPath;
  ignoredFields;
  fieldPatternPredicate = stubFalse;
  canSupportSpecificationExtensions = false;
  specificationExtensionPredicate = isOpenApiExtension;
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
      } else if (!this.ignoredFields.includes(toValue(key)) && this.fieldPatternPredicate(toValue(key))) {
        const specPath = this.specPath(value);
        const patternedFieldElement = this.toRefractedElement(specPath, value);
        const newMemberElement = new MemberElement(cloneDeep(key), patternedFieldElement);
        this.copyMetaAndAttributes(memberElement, newMemberElement);
        newMemberElement.classes.push('patterned-field');
        this.element.content.push(newMemberElement);
      } else if (!this.ignoredFields.includes(toValue(key))) {
        this.element.content.push(cloneDeep(memberElement));
      }
    });
    this.copyMetaAndAttributes(objectElement, this.element);
    return BREAK;
  }
}
export default PatternedFieldsVisitor;