import { Mixin } from 'ts-mixer';
import { isStringElement, BREAK, cloneDeep } from '@swagger-api/apidom-core';
import ChannelItemServersElement from "../../../../elements/nces/ChannelItemsServers.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ServersVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ChannelItemServersElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const element = cloneDeep(item);
      if (isStringElement(element)) {
        element.classes.push('server-name');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default ServersVisitor;