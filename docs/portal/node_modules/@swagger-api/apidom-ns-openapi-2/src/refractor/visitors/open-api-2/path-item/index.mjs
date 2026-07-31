import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { isStringElement, cloneDeep, toValue } from '@swagger-api/apidom-core';
import PathItemElement from "../../../../elements/PathItem.mjs";
import { isOperationElement } from "../../../../predicates.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PathItemVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new PathItemElement();
    this.specPath = always(['document', 'objects', 'PathItem']);
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate Operation elements with HTTP method
    this.element.filter(isOperationElement)
    // @ts-ignore
    .forEach((operationElement, httpMethodElementCI) => {
      const httpMethodElementCS = cloneDeep(httpMethodElementCI);
      httpMethodElementCS.content = toValue(httpMethodElementCS).toUpperCase();
      operationElement.setMetaProperty('http-method', httpMethodElementCS);
    });

    // mark this PathItemElement with reference metadata
    if (isStringElement(this.element.$ref)) {
      this.element.classes.push('reference-element');
    }
    return result;
  }
}
export default PathItemVisitor;