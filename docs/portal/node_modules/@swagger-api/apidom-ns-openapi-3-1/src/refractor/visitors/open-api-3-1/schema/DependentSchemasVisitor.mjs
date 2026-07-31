import { DependentSchemasVisitor as DependentSchemaJSONSchema202012Visitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class DependentSchemasVisitor extends DependentSchemaJSONSchema202012Visitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default DependentSchemasVisitor;