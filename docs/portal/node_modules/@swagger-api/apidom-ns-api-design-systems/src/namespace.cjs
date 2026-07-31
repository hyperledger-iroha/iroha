"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _Main = _interopRequireDefault(require("./elements/Main.cjs"));
var _Principle = _interopRequireDefault(require("./elements/Principle.cjs"));
var _Requirement = _interopRequireDefault(require("./elements/Requirement.cjs"));
var _RequirementLevel = _interopRequireDefault(require("./elements/RequirementLevel.cjs"));
var _Scenario = _interopRequireDefault(require("./elements/Scenario.cjs"));
var _Standard = _interopRequireDefault(require("./elements/Standard.cjs"));
var _StandardIdentifier = _interopRequireDefault(require("./elements/StandardIdentifier.cjs"));
/**
 * API Design Systems 2021-05-07 specification elements.
 */

/**
 * @public
 */
const apiDesignSystems = {
  namespace: options => {
    const {
      base
    } = options;

    /**
     * API Design Systems 2021-05-07 specification elements.
     */
    base.register('info', _Info.default);
    base.register('main', _Main.default);
    base.register('principle', _Principle.default);
    base.register('requirement', _Requirement.default);
    base.register('requirementLevel', _RequirementLevel.default);
    base.register('scenario', _Scenario.default);
    base.register('standard', _Standard.default);
    base.register('standardIdentifier', _StandardIdentifier.default);
    return base;
  }
};
var _default = exports.default = apiDesignSystems;