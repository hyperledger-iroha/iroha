import { includes } from 'ramda';
import { visit, toValue } from '@swagger-api/apidom-core';
import { getNodeType, keyMap } from '@swagger-api/apidom-ns-openapi-3-1';
/**
 * This file contains logic for translating Standard Identifier to list of Operation Elements.
 */

const visitorOptions = {
  keyMap,
  nodeTypeGetter: getNodeType
};
const select = (openAPIElement, standardIdentifier) => {
  const selected = [];
  const visitor = {
    OperationElement(element) {
      if (!element.meta.hasKey('ads-s-standard-identifier')) return;
      const standardIdentifiers = toValue(element.meta.get('ads-s-standard-identifier'));
      if (includes(toValue(standardIdentifier), standardIdentifiers)) {
        selected.push(element);
      }
    },
    ResponseElement(element) {
      if (!element.meta.hasKey('ads-s-standard-identifier')) return;
      const standardIdentifiers = toValue(element.meta.get('ads-s-standard-identifier'));
      if (includes(toValue(standardIdentifier), standardIdentifiers)) {
        selected.push(element);
      }
    }
  };
  visit(openAPIElement, visitor, visitorOptions);
  return selected;
};
export default select;