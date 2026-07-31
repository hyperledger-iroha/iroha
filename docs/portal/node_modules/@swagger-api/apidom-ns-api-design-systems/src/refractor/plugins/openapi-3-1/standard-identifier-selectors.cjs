"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
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
          operationIdentifiers.push(['http', 'transaction'], ['http', 'request'], ['http', 'request', 'url'], ['http', 'request', 'url', (0, _apidomCore.toValue)(parentPathItem.meta.get('path'))], ['http', 'request', 'method'], ['http', 'request', 'method', (0, _apidomCore.toValue)(element.meta.get('http-method')).toLowerCase()]);

          // fold PathItem.parameters to Operation.parameters
          // @ts-ignore
          parentPathItem == null || (_parentPathItem$param = parentPathItem.parameters) == null || _parentPathItem$param.forEach(parameter => {
            if ((0, _apidomNsOpenapi.isStringElement)(parameter.in) && (0, _apidomNsOpenapi.isStringElement)(parameter.name) && (0, _apidomCore.toValue)(parameter.in) === 'header') {
              operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', (0, _apidomCore.toValue)(parameter.name)], ['http', 'message', 'header'], ['http', 'message', 'header', (0, _apidomCore.toValue)(parameter.name)]);
            }
          });
        },
        leave(element) {
          element.setMetaProperty('ads-s-standard-identifier', operationIdentifiers);
          operationIdentifiers = [];
        }
      },
      ParameterElement(element) {
        if ((0, _apidomNsOpenapi.isStringElement)(element.in) && (0, _apidomNsOpenapi.isStringElement)(element.name) && (0, _apidomCore.toValue)(element.in) === 'header') {
          operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', (0, _apidomCore.toValue)(element.name)], ['http', 'message', 'header'], ['http', 'message', 'header', (0, _apidomCore.toValue)(element.name)]);
        }
      },
      RequestBodyElement(element) {
        if (!(0, _apidomNsOpenapi.isObjectElement)(element.contentProp)) return;
        operationIdentifiers.push(['http', 'request', 'body'], ['http', 'message', 'body']);
        if (typeof element.contentProp !== 'undefined' && (0, _apidomNsOpenapi.isObjectElement)(element.contentProp)) {
          element.contentProp.forEach(() => {
            operationIdentifiers.push(['http', 'request', 'header'], ['http', 'request', 'header', 'Content-Type'], ['http', 'message', 'header'], ['http', 'message', 'header', 'Content-Type']);
          });
        }
      },
      ResponseElement: {
        enter(element) {
          responseIdentifiers.push(['http', 'response']);
          if (element.meta.hasKey('http-status-code')) {
            const statusCode = String((0, _apidomCore.toValue)(element.meta.get('http-status-code')));
            const statusCodeAlias = statusCode.startsWith('2') ? 'success' : statusCode.startsWith('3') ? 'redirect' : statusCode.startsWith('4') ? 'client_error' : statusCode.startsWith('5') ? 'sever_error' : 'unknown';
            responseIdentifiers.push(['http', 'response', 'status_code'], ['http', 'response', 'status_code', statusCode], ['http', 'response', 'status_code', statusCodeAlias]);
          }
          if (typeof element.headers !== 'undefined' && (0, _apidomNsOpenapi.isObjectElement)(element.headers)) {
            element.headers.forEach((value, key) => {
              const headerName = (0, _apidomCore.toValue)(key);
              responseIdentifiers.push(['http', 'response', 'header'], ['http', 'response', 'header', headerName], ['http', 'message', 'header', headerName]);
            });
          }
          if (typeof element.contentProp !== 'undefined' && (0, _apidomNsOpenapi.isObjectElement)(element.contentProp)) {
            responseIdentifiers.push(['http', 'response', 'body'], ['http', 'message', 'body']);
            element.contentProp.forEach((value, key) => {
              const headerName = (0, _apidomCore.toValue)(key);
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
var _default = exports.default = plugin;