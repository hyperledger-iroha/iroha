import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import InfoElement from "../../../../elements/Info.mjs";
export const BaseInfoVisitor = AsyncApi2Specification.visitors.document.objects.Info.$visitor;
/**
 * @public
 */
class InfoVisitor extends BaseInfoVisitor {
  constructor(options) {
    super(options);
    this.element = new InfoElement();
  }
}
export default InfoVisitor;