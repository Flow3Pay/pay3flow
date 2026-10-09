/** The injected EVER Wallet RPC API, including its raw string addresses.
 * https://docs.broxus.com/pages/raw/provider-api.html
 */
export interface EverscaleProvider {
  request(args: { method: "getProviderState" }): Promise<EverscaleState>;
  request(args: { method: "requestPermissions"; params: { permissions: ["basic", "accountInteraction"] } }): Promise<EverscalePermissions>;
  request(args: { method: "disconnect" }): Promise<unknown>;
  addListener(event: string, listener: (value: unknown) => void): unknown;
  removeListener(event: string, listener: (value: unknown) => void): unknown;
}

interface EverscalePermissions {
  accountInteraction?: { address: string };
}

interface EverscaleState {
  selectedConnection: string;
  networkId: number;
  permissions: EverscalePermissions;
}

export interface EverscaleConnection {
  family: "everscale";
  address: string;
  networkId: number;
  provider: EverscaleProvider;
}

export function isEverscaleAddress(address: string): boolean {
  return /^(?:0|-1):[\da-f]{64}$/i.test(address);
}

async function injectedProvider(): Promise<EverscaleProvider> {
  const browser = window as Window & { __ever?: EverscaleProvider; __hasEverscaleProvider?: boolean };
  if (browser.__ever) return browser.__ever;
  if (document.readyState !== "complete") {
    await new Promise<void>(resolve => window.addEventListener("load", () => resolve(), { once: true }));
  }
  if (browser.__ever) return browser.__ever;
  if (!browser.__hasEverscaleProvider) throw new Error("Install EVER Wallet, or open Pay3Flow in its browser, to connect Everscale.");
  // EVER Wallet can advertise itself before its RPC object is initialized.
  return new Promise((resolve, reject) => {
    const cleanup = () => { window.clearTimeout(timer); window.removeEventListener("ever#initialized", initialized); };
    const initialized = () => {
      if (!browser.__ever) return;
      cleanup();
      resolve(browser.__ever);
    };
    const timer = window.setTimeout(() => { cleanup(); reject(new Error("EVER Wallet did not initialize. Unlock it and try again.")); }, 10_000);
    window.addEventListener("ever#initialized", initialized);
    initialized();
  });
}

function connection(provider: EverscaleProvider, state: EverscaleState): EverscaleConnection | null {
  const address = state.permissions.accountInteraction?.address;
  // The documented connection group distinguishes Everscale from other TVM
  // networks offered by EVER Wallet. Keep the reported numeric id for updates.
  if (state.selectedConnection !== "mainnet" || !Number.isInteger(state.networkId) || !address || !isEverscaleAddress(address)) return null;
  return { family: "everscale", address: address.toLowerCase(), networkId: state.networkId, provider };
}

export async function connectEverscale(): Promise<EverscaleConnection> {
  const provider = await injectedProvider();
  const initial = await provider.request({ method: "getProviderState" });
  if (initial.selectedConnection !== "mainnet") throw new Error("Switch EVER Wallet to Everscale mainnet and try again.");
  await provider.request({ method: "requestPermissions", params: { permissions: ["basic", "accountInteraction"] } });
  const state = await provider.request({ method: "getProviderState" });
  if (state.selectedConnection !== "mainnet") throw new Error("Switch EVER Wallet to Everscale mainnet and try again.");
  const wallet = connection(provider, state);
  if (!wallet) throw new Error("Unlock EVER Wallet and authorize this website.");
  return wallet;
}

/** Restore permissions without requesting access or opening a wallet prompt. */
export async function restoreEverscale(): Promise<EverscaleConnection | null> {
  const provider = await injectedProvider();
  return connection(provider, await provider.request({ method: "getProviderState" }));
}

export function observeEverscale(wallet: EverscaleConnection, changed: (wallet: EverscaleConnection | null) => void): () => void {
  let revision = 0;
  let active = true;
  const refresh = async () => {
    const current = ++revision;
    try {
      const state = await wallet.provider.request({ method: "getProviderState" });
      if (active && current === revision) changed(connection(wallet.provider, state));
    } catch {
      if (active && current === revision) changed(null);
    }
  };
  const clear = () => { ++revision; if (active) changed(null); };
  const events = ["permissionsChanged", "networkChanged", "connected"];
  const clears = ["disconnected", "loggedOut"];
  for (const event of events) wallet.provider.addListener(event, refresh);
  for (const event of clears) wallet.provider.addListener(event, clear);
  // Reconcile changes that occurred between connection and listener setup.
  void refresh();
  return () => {
    active = false;
    ++revision;
    for (const event of events) wallet.provider.removeListener(event, refresh);
    for (const event of clears) wallet.provider.removeListener(event, clear);
  };
}
