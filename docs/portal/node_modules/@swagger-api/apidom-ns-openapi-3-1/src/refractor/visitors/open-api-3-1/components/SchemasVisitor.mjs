import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { toValue } from '@swagger-api/apidom-core';
import { ComponentsSchemasElement, MapVisitor, FallbackVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { isSchemaElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SchemasVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsSchemasElement();
    this.specPath = always(['document', 'objects', 'Schema']);
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate Schemas elements with Schema name
    this.element.filter(isSchemaElement)
    // @ts-ignore
    .forEach((schemaElement, schemaName) => {
      schemaElement.setMetaProperty('schemaName', toValue(schemaName));
    });
    return result;
  }
}
export default SchemasVisitor;