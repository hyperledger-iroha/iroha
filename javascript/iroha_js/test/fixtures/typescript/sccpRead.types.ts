import { ToriiClient, ToriiBrowserClient, type SccpCapabilitiesV1, type SccpRecentMessagesV1 } from "../../../index.js";
import type { SccpRouteV1 } from "../../../torii.js";
declare const node: ToriiClient;
declare const browser: ToriiBrowserClient;
const capabilities: Promise<SccpCapabilitiesV1>[] = [node.getSccpCapabilitiesV1(), browser.getSccpCapabilitiesV1()];
const routes: Promise<SccpRouteV1[]>[] = [node.getSccpRegistryV1()];
const pages: Promise<SccpRecentMessagesV1>[] = [node.listSccpRecentMessagesV1({ limit: 50 }), node.listSccpRecentMessagesV1({ direction: "inbound", network: "ton-mainnet" })];
void [capabilities, routes, pages];
// @ts-expect-error The removed network has no current profile.
node.listSccpRecentMessagesV1({ network: "tron-mainnet" });
// @ts-expect-error A numeric string is not a bounded page size.
node.listSccpRecentMessagesV1({ limit: "10" });
// @ts-expect-error This public read does not accept authentication overrides.
node.getSccpCapabilitiesV1({ canonicalAuth: {} });

// @ts-expect-error Browser registry reads require a native account codec.
browser.getSccpRegistryV1();
// @ts-expect-error Browser message reads require a native account codec.
browser.listSccpRecentMessagesV1();
