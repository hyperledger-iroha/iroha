import { PrefixItemsVisitor as PrefixItemsJSONSchema202012Visitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class PrefixItemsVisitor extends PrefixItemsJSONSchema202012Visitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default PrefixItemsVisitor;