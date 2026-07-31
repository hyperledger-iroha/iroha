import { toValue } from '@swagger-api/apidom-core';
import { isStringElement, isObjectElement } from '@swagger-api/apidom-ns-openapi-3-1';

/**
 * @public
 */
const plugin = () => () => {
  let operationIdentifiers = [];
  let responseIdentifiers = [];
  return {
    visitor: {
      OperationElement: {
        enter(element, ...rest) {
          var _parentPathItem$param;
          const [,,, ancestors] = rest;
          const parentPathItem = ancestors[ancestors.length - 2];
          operationIdentifiers.push(['http', 'transaction'], ['http', 'request'], ['http', 'request', 'url'], ['http', 'request', 'url', toValue(parentPathItem.meta.get('path'))], ['http', 'request', 'method'], ['http', 'request', 'method', toValue(element.meta.get('http-method')).toLowerCase()]);

          // fold PathItem.parameters to Operation.parameters
          // @ts-ignore
          parentPathItem === null || parentPathItem === void 0 || (_parentPathItem$param = parentPathItem.parameters) === null || _parentPathItem$param === void 0 || _parentPathItem$param.forEach(parameter => {
            if (isStringElement(parameter.in) && isStringElement(parameter.name) && toValue(parameter.in) === 'header') {
              operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', toValue(parameter.name)], ['http', 'message', 'header'], ['http', 'message', 'header', toValue(parameter.name)]);
            }
          });
        },
        leave(element) {
          element.setMetaProperty('ads-s-standard-identifier', operationIdentifiers);
          operationIdentifiers = [];
        }
      },
      ParameterElement(element) {
        if (isStringElement(element.in) && isStringElement(element.name) && toValue(element.in) === 'header') {
          operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', toValue(element.name)], ['http', 'message', 'header'], ['http', 'message', 'header', toValue(element.name)]);
        }
      },
      RequestBodyElement(element) {
        if (!isObjectElement(element.contentProp)) return;
        operationIdentifiers.push(['http', 'request', 'body'], ['http', 'message', 'body']);
        if (typeof element.contentProp !== 'undefined' && isObjectElement(element.contentProp)) {
          element.contentProp.forEach(() => {
            operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', 'Content-Type'], ['http', 'message', 'header'], ['http', 'message', 'header', 'Content-Type']);
          });
        }
      },
      ResponseElement: {
        enter(element) {
          responseIdentifiers.push(['http', 'response']);
          if (element.meta.hasKey('http-status-code')) {
            const statusCode = String(toValue(element.meta.get('http-status-code')));
            const statusCodeAlias = statusCode.startsWith('2') ? 'success' : statusCode.startsWith('3') ? 'redirect' : statusCode.startsWith('4') ? 'client_error' : statusCode.startsWith('5') ? 'sever_error' : 'unknown';
            responseIdentifiers.push(['http', 'response', 'status_code'], ['http', 'response', 'status_code', statusCode], ['http', 'response', 'status_code', statusCodeAlias]);
          }
          if (typeof element.headers !== 'undefined' && isObjectElement(element.headers)) {
            element.headers.forEach((value, key) => {
              const headerName = toValue(key);
              responseIdentifiers.push(['http', 'response', 'header'], ['http', 'response', 'header', headerName], ['http', 'message', 'header', headerName]);
            });
          }
          if (typeof element.contentProp !== 'undefined' && isObjectElement(element.contentProp)) {
            responseIdentifiers.push(['http', 'response', 'body'], ['http', 'message', 'body']);
            element.contentProp.forEach((value, key) => {
              const headerName = toValue(key);
              responseIdentifiers.push(['http', 'response', 'header'], ['http', 'response', 'header', headerName], ['http', 'message', 'header', headerName]);
            });
          }
        },
        leave(element) {
          element.setMetaProperty('ads-s-standard-identifier', responseIdentifiers);
          responseIdentifiers = [];
        }
      }
    }
  };
};
export default plugin;