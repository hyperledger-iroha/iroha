import cookieValueStrictBase64urlEncoder from "./encoders/cookie-value-strict-base64url.mjs";
import cookieNameStrictValidator from "./validators/cookie-name-strict.mjs";
import cookieValueStrictValidator from "./validators/cookie-value-strict.mjs";
import { identity } from "../../utils.mjs";
const defaultOptions = {
  encoders: {
    name: identity,
    value: cookieValueStrictBase64urlEncoder
  },
  validators: {
    name: cookieNameStrictValidator,
    value: cookieValueStrictValidator
  }
};
const serialize = (name, value, options = {}) => {
  const mergedOptions = {
    ...defaultOptions,
    ...options,
    encoders: {
      ...defaultOptions.encoders,
      ...options.encoders
    },
    validators: {
      ...defaultOptions.validators,
      ...options.validators
    }
  };
  const encodedName = mergedOptions.encoders.name(name);
  const encodedValue = mergedOptions.encoders.value(value);
  mergedOptions.validators.name(encodedName);
  mergedOptions.validators.value(encodedValue);
  return `${encodedName}=${encodedValue}`;
};
export default serialize;