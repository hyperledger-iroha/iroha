import { createRefractor } from "./index.mjs";
/**
 * API Design Systems 2021-05-07 specification elements.
 */
import MainElement from "../elements/Main.mjs";
import InfoElement from "../elements/Info.mjs";
import PrincipleElement from "../elements/Principle.mjs";
import RequirementElement from "../elements/Requirement.mjs";
import RequirementLevelElement from "../elements/RequirementLevel.mjs";
import ScenarioElement from "../elements/Scenario.mjs";
import StandardElement from "../elements/Standard.mjs";
import StandardIdentifierElement from "../elements/StandardIdentifier.mjs";
/**
 * API Design Systems 2021-05-07 specification elements.
 */
MainElement.refract = createRefractor(['visitors', 'document', 'objects', 'Main', '$visitor']);
InfoElement.refract = createRefractor(['visitors', 'document', 'objects', 'Info', '$visitor']);
PrincipleElement.refract = createRefractor(['visitors', 'document', 'objects', 'Principle', '$visitor']);
RequirementElement.refract = createRefractor(['visitors', 'document', 'objects', 'Requirement', '$visitor']);
RequirementLevelElement.refract = createRefractor(['visitors', 'document', 'objects', 'RequirementLevel', '$visitor']);
ScenarioElement.refract = createRefractor(['visitors', 'document', 'objects', 'Scenario', '$visitor']);
StandardElement.refract = createRefractor(['visitors', 'document', 'objects', 'Standard', '$visitor']);
StandardIdentifierElement.refract = createRefractor(['visitors', 'document', 'objects', 'StandardIdentifier', '$visitor']);
export { MainElement, InfoElement, PrincipleElement, RequirementElement, RequirementLevelElement, ScenarioElement, StandardElement, StandardIdentifierElement };