"use strict";
/**
 * @prettier
 */
"use client";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _react = _interopRequireWildcard(require("react"));
var _propTypes = _interopRequireDefault(require("prop-types"));
var _swaggerUi = _interopRequireDefault(require("#swagger-ui"));
const {
  config
} = _swaggerUi.default;
const usePrevious = value => {
  const ref = (0, _react.useRef)();
  (0, _react.useEffect)(() => {
    ref.current = value;
  }, [value]);
  return ref.current;
};
const SwaggerUI = ({
  spec = config.defaults.spec,
  url = config.defaults.url,
  layout = config.defaults.layout,
  requestInterceptor = config.defaults.requestInterceptor,
  responseInterceptor = config.defaults.responseInterceptor,
  supportedSubmitMethods = config.defaults.supportedSubmitMethods,
  queryConfigEnabled = config.defaults.queryConfigEnabled,
  plugins = config.defaults.plugins,
  displayOperationId = config.defaults.displayOperationId,
  showMutatedRequest = config.defaults.showMutatedRequest,
  docExpansion = config.defaults.docExpansion,
  defaultModelExpandDepth = config.defaults.defaultModelExpandDepth,
  defaultModelsExpandDepth = config.defaults.defaultModelsExpandDepth,
  defaultModelRendering = config.defaults.defaultModelRendering,
  presets = config.defaults.presets,
  deepLinking = config.defaults.deepLinking,
  showExtensions = config.defaults.showExtensions,
  showCommonExtensions = config.defaults.showCommonExtensions,
  filter = config.defaults.filter,
  requestSnippetsEnabled = config.defaults.requestSnippetsEnabled,
  requestSnippets = config.defaults.requestSnippets,
  tryItOutEnabled = config.defaults.tryItOutEnabled,
  displayRequestDuration = config.defaults.displayRequestDuration,
  withCredentials = config.defaults.withCredentials,
  persistAuthorization = config.defaults.persistAuthorization,
  oauth2RedirectUrl = config.defaults.oauth2RedirectUrl,
  onComplete = null,
  initialState = config.defaults.initialState,
  uncaughtExceptionHandler = config.defaults.uncaughtExceptionHandler
}) => {
  const [system, setSystem] = (0, _react.useState)(null);
  const SwaggerUIComponent = system?.getComponent("App", "root");
  const prevSpec = usePrevious(spec);
  const prevUrl = usePrevious(url);
  (0, _react.useEffect)(() => {
    const systemInstance = (0, _swaggerUi.default)({
      plugins,
      spec,
      url,
      layout,
      defaultModelsExpandDepth,
      defaultModelRendering,
      presets: [_swaggerUi.default.presets.apis, ...presets],
      requestInterceptor,
      responseInterceptor,
      onComplete: () => {
        if (typeof onComplete === "function") {
          onComplete(systemInstance);
        }
      },
      docExpansion,
      supportedSubmitMethods,
      queryConfigEnabled,
      defaultModelExpandDepth,
      displayOperationId,
      tryItOutEnabled,
      displayRequestDuration,
      requestSnippetsEnabled,
      requestSnippets,
      showMutatedRequest,
      deepLinking,
      showExtensions,
      showCommonExtensions,
      filter,
      persistAuthorization,
      withCredentials,
      initialState,
      uncaughtExceptionHandler,
      ...(typeof oauth2RedirectUrl === "string" ? {
        oauth2RedirectUrl: oauth2RedirectUrl
      } : {})
    });
    setSystem(systemInstance);
  }, []);
  (0, _react.useEffect)(() => {
    if (system) {
      const prevStateUrl = system.specSelectors.url();
      if (url !== prevStateUrl || url !== prevUrl) {
        system.specActions.updateSpec("");
        if (url) {
          system.specActions.updateUrl(url);
          system.specActions.download(url);
        }
      }
    }
  }, [system, url]);
  (0, _react.useEffect)(() => {
    if (system) {
      const prevStateSpec = system.specSelectors.specStr();
      if (spec && spec !== _swaggerUi.default.config.defaults.spec && (spec !== prevStateSpec || spec !== prevSpec)) {
        const updatedSpec = typeof spec === "object" ? JSON.stringify(spec) : spec;
        system.specActions.updateSpec(updatedSpec);
      }
    }
  }, [system, spec]);
  return SwaggerUIComponent ? /*#__PURE__*/_react.default.createElement(SwaggerUIComponent, null) : null;
};
SwaggerUI.System = _swaggerUi.default.System;
SwaggerUI.presets = _swaggerUi.default.presets;
SwaggerUI.plugins = _swaggerUi.default.plugins;
SwaggerUI.config = _swaggerUi.default.config;
var _default = exports.default = SwaggerUI;

