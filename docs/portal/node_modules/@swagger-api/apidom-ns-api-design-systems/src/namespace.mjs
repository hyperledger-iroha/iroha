/**
 * API Design Systems 2021-05-07 specification elements.
 */
import InfoElement from "./elements/Info.mjs";
import MainElement from "./elements/Main.mjs";
import PrincipleElement from "./elements/Principle.mjs";
import RequirementElement from "./elements/Requirement.mjs";
import RequirementLevelElement from "./elements/RequirementLevel.mjs";
import ScenarioElement from "./elements/Scenario.mjs";
import StandardElement from "./elements/Standard.mjs";
import StandardIdentifierElement from "./elements/StandardIdentifier.mjs";
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
    base.register('info', InfoElement);
    base.register('main', MainElement);
    base.register('principle', PrincipleElement);
    base.register('requirement', RequirementElement);
    base.register('requirementLevel', RequirementLevelElement);
    base.register('scenario', ScenarioElement);
    base.register('standard', StandardElement);
    base.register('standardIdentifier', StandardIdentifierElement);
    return base;
  }
};
export default apiDesignSystems;