import { getNativeBinding } from "../../src/native.js";
import { createNativeTestHelper } from "./nativeRequirements.js";

let binding = null;
let bindingError = null;
try {
  binding = getNativeBinding();
} catch (error) {
  bindingError = error;
}

export const {
  nativeBinding,
  nativeBindingError,
  hasNativeBinding,
  noritoRequiredMethods,
  sm2RequiredMethods,
  nativeUnavailableMessage,
  hasNoritoBinding,
  hasSm2Binding,
  makeNativeTest,
} = createNativeTestHelper(binding, bindingError);
