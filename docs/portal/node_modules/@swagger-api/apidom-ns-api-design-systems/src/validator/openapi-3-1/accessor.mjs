import { visit, ArrayElement, toValue } from '@swagger-api/apidom-core';
import { getNodeType, keyMap } from '@swagger-api/apidom-ns-openapi-3-1';
/**
 * This file contains logic for translating Standard Identifier to value.
 */

const visitorOptions = {
  keyMap,
  nodeTypeGetter: getNodeType
};
const access = (selected, standardIdentifier) => {
  const strStandardIdentifier = String(toValue(standardIdentifier));
  const values = new ArrayElement();
  const visitor = {
    enter(element) {
      if (!element.meta.hasKey('ads-a-standard-identifier')) return;
      element.meta.get('ads-a-standard-identifier').content.filter(accessorMapping => {
        return String(toValue(accessorMapping.get('subject'))) === strStandardIdentifier;
      }).forEach(accessorMapping => {
        values.push(accessorMapping.get('value'));
      });
    }
  };
  visit(selected, visitor, visitorOptions);
  return values;
};
export default access;