"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
/**
 * @public
 */
const plugin = () => () => {
  return {
    visitor: {
      OperationElement(element, ...rest) {
        const [,,, ancestors] = rest;
        const parentPathItem = ancestors[ancestors.length - 2];
        const httpMethod = (0, _apidomCore.cloneDeep)(element.meta.get('http-method'));
        httpMethod.content = (0, _apidomCore.toValue)(httpMethod).toLowerCase();
        const standardIdentifiers = [{
          subject: ['http', 'request', 'url'],
          value: parentPathItem.meta.get('path')
        }, {
          subject: ['http', 'request', 'method'],
          value: httpMethod
        }];
        if (typeof parentPathItem.parameters !== 'undefined' && (0, _apidomNsOpenapi.isArrayElement)(parentPathItem.parameters)) {
          // @ts-ignore
          parentPathItem.parameters.forEach(parameter => {
            if ((0, _apidomNsOpenapi.isStringElement)(parameter.in) && (0, _apidomNsOpenapi.isStringElement)(parameter.name) && (0, _apidomCore.toValue)(parameter.in) === 'header') {
              standardIdentifiers.push({
                subject: ['http', 'request', 'header'],
                value: _apidomCore.cloneDeep.safe(parameter.name)
              });
              standardIdentifiers.push({
                subject: ['http', 'message', 'header'],
                value: _apidomCore.cloneDeep.safe(parameter.name)
              });
            }
          });
        }
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      },
      ParameterElement(element) {
        if ((0, _apidomNsOpenapi.isStringElement)(element.in) && (0, _apidomNsOpenapi.isStringElement)(element.name) && (0, _apidomCore.toValue)(element.in) === 'header') {
          element.setMetaProperty('ads-a-standard-identifier', [{
            subject: ['http', 'request', 'header'],
            value: _apidomCore.cloneDeep.safe(element.name)
          }, {
            subject: ['http', 'message', 'header'],
            value: _apidomCore.cloneDeep.safe(element.name)
          }]);
        }
      },
      RequestBodyElement(element) {
        if (typeof element.contentProp === 'undefined' || !(0, _apidomNsOpenapi.isObjectElement)(element.contentProp)) {
          return;
        }
        const standardIdentifiers = [];
        element.contentProp.forEach((mediaType, key) => {
          standardIdentifiers.push({
            subject: ['http', 'request', 'header'],
            value: new _apidomCore.StringElement('Content-Type', (0, _apidomCore.cloneDeep)(key.meta))
          }, {
            subject: ['http', 'message', 'header'],
            value: new _apidomCore.StringElement('Content-Type', (0, _apidomCore.cloneDeep)(key.meta))
          }, {
            subject: ['http', 'request', 'header', 'Content-Type'],
            value: (0, _apidomCore.cloneDeep)(key)
          }, {
            subject: ['http', 'message', 'header', 'Content-Type'],
            value: (0, _apidomCore.cloneDeep)(key)
          });
        });
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      },
      ResponseElement(element) {
        if (!element.meta.hasKey('http-status-code')) return;
        const standardIdentifiers = [{
          subject: ['http', 'response', 'status_code'],
          value: (0, _apidomCore.cloneDeep)(element.meta.get('http-status-code'))
        }];
        element.setMetaProperty('ads-a-standard-identifier', standardIdentifiers);
      }
    }
  };
};
var _default = exports.default = plugin;