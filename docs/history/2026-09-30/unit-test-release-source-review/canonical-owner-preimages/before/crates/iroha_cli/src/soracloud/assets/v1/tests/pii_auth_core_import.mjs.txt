import { pathToFileURL } from "node:url";

const CORE_MODULE_PATH = __CORE_MODULE_PATH__;

process.argv = [process.argv[0], CORE_MODULE_PATH, "--port=0"];
__ENV_ASSIGNMENTS__
__SETUP_BEFORE_IMPORT__

await import(`${pathToFileURL(CORE_MODULE_PATH).href}?auth-core=__SCENARIO__`);
