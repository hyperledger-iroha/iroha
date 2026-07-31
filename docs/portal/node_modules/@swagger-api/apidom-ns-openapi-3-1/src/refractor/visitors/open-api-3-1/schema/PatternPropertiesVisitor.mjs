import { PatternPropertiesVisitor as PatternPropertiesJSONSchema202012Visitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class PatternPropertiesVisitor extends PatternPropertiesJSONSchema202012Visitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default PatternPropertiesVisitor;