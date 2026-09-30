import { pathToFileURL } from "node:url";

const SERVER_PATH = __SERVER_PATH__;
const EXPECTED = __EXPECTED_ERROR__;

process.argv = [process.argv[0], SERVER_PATH, "--port=0"];
__ENV_ASSIGNMENTS__

try {
  await import(`${pathToFileURL(SERVER_PATH).href}?startup=__SCENARIO__`);
} catch (error) {
  const logs = `${error?.stack ?? ""}\n${error?.message ?? ""}\n${String(error)}`;
  if (!logs.includes(EXPECTED)) {
    console.error(`missing expected startup error. expected=${EXPECTED} logs=${logs}`);
    process.exit(1);
  }
  process.exit(0);
}

console.error(`pii-app server unexpectedly started; expected startup error: ${EXPECTED}`);
process.exit(1);
