import { PropertiesVisitor as PropertiesJSONSchema202012Visitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class PropertiesVisitor extends PropertiesJSONSchema202012Visitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default PropertiesVisitor;