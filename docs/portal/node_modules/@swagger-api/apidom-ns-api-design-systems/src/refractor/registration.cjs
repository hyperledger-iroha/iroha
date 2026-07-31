"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
var _index = require("./index.cjs");
var _Main = _interopRequireDefault(require("../elements/Main.cjs"));
exports.MainElement = _Main.default;
var _Info = _interopRequireDefault(require("../elements/Info.cjs"));
exports.InfoElement = _Info.default;
var _Principle = _interopRequireDefault(require("../elements/Principle.cjs"));
exports.PrincipleElement = _Principle.default;
var _Requirement = _interopRequireDefault(require("../elements/Requirement.cjs"));
exports.RequirementElement = _Requirement.default;
var _RequirementLevel = _interopRequireDefault(require("../elements/RequirementLevel.cjs"));
exports.RequirementLevelElement = _RequirementLevel.default;
var _Scenario = _interopRequireDefault(require("../elements/Scenario.cjs"));
exports.ScenarioElement = _Scenario.default;
var _Standard = _interopRequireDefault(require("../elements/Standard.cjs"));
exports.StandardElement = _Standard.default;
var _StandardIdentifier = _interopRequireDefault(require("../elements/StandardIdentifier.cjs"));
exports.StandardIdentifierElement = _StandardIdentifier.default;
/**
 * API Design Systems 2021-05-07 specification elements.
 */

/**
 * API Design Systems 2021-05-07 specification elements.
 */
_Main.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Main', '$visitor']);
_Info.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Info', '$visitor']);
_Principle.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Principle', '$visitor']);
_Requirement.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Requirement', '$visitor']);
_RequirementLevel.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'RequirementLevel', '$visitor']);
_Scenario.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Scenario', '$visitor']);
_Standard.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'Standard', '$visitor']);
_StandardIdentifier.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'StandardIdentifier', '$visitor']);