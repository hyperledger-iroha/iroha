import { $defsVisitor as $defsJSONSchema202012Visitor } from '@swagger-api/apidom-ns-json-schema-2020-12';

/**
 * @public
 */

/**
 * @public
 */
class $defsVisitor extends $defsJSONSchema202012Visitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default $defsVisitor;