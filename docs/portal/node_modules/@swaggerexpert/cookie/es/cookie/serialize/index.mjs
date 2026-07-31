import serializeSetCookie from "../../set-cookie/serialize/index.mjs";
const serialize = (cookiePairs, options = {}) => {
  const cookiePairEntries = Array.isArray(cookiePairs) ? cookiePairs : typeof cookiePairs === 'object' && cookiePairs !== null ? Object.entries(cookiePairs) : [];
  return cookiePairEntries.map(([name, value]) => serializeSetCookie(name, value, options)).join('; ');
};
export default serialize;