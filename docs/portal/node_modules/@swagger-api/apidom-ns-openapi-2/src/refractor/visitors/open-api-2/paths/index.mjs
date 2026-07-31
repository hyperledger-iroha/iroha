import { Mixin } from 'ts-mixer';
import { T as stubTrue, always } from 'ramda';
import { cloneDeep } from '@swagger-api/apidom-core';
import PathsElement from "../../../../elements/Paths.mjs";
import PatternedFieldsVisitor from "../../generics/PatternedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isPathItemElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PathsVisitor extends Mixin(PatternedFieldsVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new PathsElement();
    this.specPath = always(['document', 'objects', 'PathItem']);
    this.canSupportSpecificationExtensions = true;
    this.fieldPatternPredicate = stubTrue;
  }
  ObjectElement(objectElement) {
    const result = PatternedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every PathItemElement with path metadata
    this.element.filter(isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      key.classes.push('openapi-path-template');
      key.classes.push('path-template');
      pathItemElement.setMetaProperty('path', cloneDeep(key));
    });
    return result;
  }
}
export default PathsVisitor;