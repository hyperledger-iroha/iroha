import type {
  ToriiVpnProfile,
  ToriiVpnQuote,
  ToriiVpnSession,
} from "../../../index.js";

type HasRequiredRelayMldsa65Key<T extends { relayMldsa65PublicKeyHex: string }> =
  Record<string, never> extends Pick<T, "relayMldsa65PublicKeyHex"> ? false : true;

const profileHasRequiredRelayMldsa65Key: HasRequiredRelayMldsa65Key<ToriiVpnProfile> = true;
const quoteHasRequiredRelayMldsa65Key: HasRequiredRelayMldsa65Key<ToriiVpnQuote> = true;
const sessionHasRequiredRelayMldsa65Key: HasRequiredRelayMldsa65Key<ToriiVpnSession> = true;

void profileHasRequiredRelayMldsa65Key;
void quoteHasRequiredRelayMldsa65Key;
void sessionHasRequiredRelayMldsa65Key;
