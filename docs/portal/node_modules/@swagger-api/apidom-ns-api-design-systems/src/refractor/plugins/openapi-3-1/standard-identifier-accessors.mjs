import { StringElement, toValue, cloneDeep } from '@swagger-api/apidom-core';
import { isStringElement, isArrayElement, isObjectElement } from '@swagger-api/apidom-ns-openapi-3-1';

/**
 * @public
 */
const plugin = () => () => {
  return {
    visitor: {
      OperationElement(element, ...rest) {
        const [,,, ancestors] = rest;
        const parentPathItem = ancestors[ancestors.length - 2];
        const httpMethod = cloneDeep(element.meta.get('http-method'));
        httpMethod.content = toValue(httpMethod).toLowerCase();
        const standardIdentifiers = [{
          subject: ['http', 'request', 'url'],
          value: parentPathItem.meta.get('path')
        }, {
          subject: ['http', 'request', 'method'],
          value: httpMethod
        }];
        if (typeof parentPathItem.parameters !== 'undefined' && isArrayElement(parentPathItem.parameters)) {
          // @ts-ignore
          parentPathItem.parameters.forEach(parameter => {
            if (isStringElement(parameter.in) && isStringElement(parameter.name) && toValue(parameter.in) === 'header') {
              standardIdentifiers.push({
                subject: ['http', 'request', 'header'],
                value: cloneDeep.safe(parameter.name)
              });
              standardIdentifiers.push({
                subject: ['http', 'message', 'header'],
                value: cloneDeep.safe(parameter.name)
              });
            }
          });
        }
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      },
      ParameterElement(element) {
        if (isStringElement(element.in) && isStringElement(element.name) && toValue(element.in) === 'header') {
          element.setMetaProperty('ads-a-standard-identifier', [{
            subject: ['http', 'request', 'header'],
            value: cloneDeep.safe(element.name)
          }, {
            subject: ['http', 'message', 'header'],
            value: cloneDeep.safe(element.name)
          }]);
        }
      },
      RequestBodyElement(element) {
        if (typeof element.contentProp === 'undefined' || !isObjectElement(element.contentProp)) {
          return;
        }
        const standardIdentifiers = [];
        element.contentProp.forEach((mediaType, key) => {
          standardIdentifiers.push({
            subject: ['http', 'request', 'header'],
            value: new StringElement('Content-Type', cloneDeep(key.meta))
          }, {
            subject: ['http', 'message', 'header'],
            value: new StringElement('Content-Type', cloneDeep(key.meta))
          }, {
            subject: ['http', 'request', 'header', 'Content-Type'],
            value: cloneDeep(key)
          }, {
            subject: ['http', 'message', 'header', 'Content-Type'],
            value: cloneDeep(key)
          });
        });
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      },
      ResponseElement(element) {
        if (!element.meta.hasKey('http-status-code')) return;
        const standardIdentifiers = [{
          subject: ['http', 'response', 'status_code'],
          value: cloneDeep(element.meta.get('http-status-code'))
        }];
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      }
    }
  };
};
export default plugin;