import { concat, encodeFunctionData, encodePacked, getAddress, getCreate2Address, isAddress, keccak256, pad, parseAbi, zeroAddress, type Address, type Hex } from "viem";
import { mainnet, sepolia } from "viem/chains";

export type SafeChainId = 1 | 11155111;
export const SAFE_NETWORKS = {
  1: { chain: mainnet, name: "Ethereum Mainnet", walletNetwork: "ethereum", rpcUrl: "https://ethereum-rpc.publicnode.com", explorerUrl: "https://etherscan.io", safePrefix: "eth", faucetUrl: null },
  11155111: { chain: sepolia, name: "Sepolia (testnet)", walletNetwork: "sepolia", rpcUrl: "https://ethereum-sepolia-rpc.publicnode.com", explorerUrl: "https://sepolia.etherscan.io", safePrefix: "sep", faucetUrl: "https://cloud.google.com/application/web3/faucet/ethereum/sepolia" },
} as const;

export function safeNetwork(chainId: SafeChainId) {
  if (chainId !== 1 && chainId !== 11155111) throw new Error("Unsupported Safe network.");
  return SAFE_NETWORKS[chainId];
}

// Canonical Mainnet and Sepolia deployments from safe-global/safe-deployments, v1.4.1.
export const SAFE_FACTORY = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67";
export const SAFE_SINGLETON = "0x41675C099F32341bf84BFc5382aF534df5C7461a";
export const SAFE_FALLBACK = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99";
export const SAFE_DEPLOYMENTS = [
  [SAFE_FACTORY, "0x50c3cdc4074750a7a974204a716c999edd37482f907608d960b2b025ee0b3317"],
  [SAFE_SINGLETON, "0x1fe2df852ba3299d6534ef416eefa406e56ced995bca886ab7a553e6d0c5e1c4"],
  [SAFE_FALLBACK, "0x7c6007a5d711cea8dfd5d91f5940ec29c7f200fe511eb1fc1397b367af3c42f9"],
] as const;
export const safeAbi = parseAbi([
  "function setup(address[] owners,uint256 threshold,address to,bytes data,address fallbackHandler,address paymentToken,uint256 payment,address payable paymentReceiver)",
  "function getOwners() view returns (address[])", "function getThreshold() view returns (uint256)",
]);
export const factoryAbi = parseAbi([
  "function proxyCreationCode() view returns (bytes)",
  "function createProxyWithNonce(address singleton,bytes initializer,uint256 saltNonce) returns (address proxy)",
]);
export function safeOwners(values: string[]): Address[] {
  if (values.length !== 3 || values.some(value => !isAddress(value) || value.toLowerCase() === zeroAddress)) throw new Error("Enter three valid nonzero Ethereum owner addresses.");
  const owners = values.map(value => getAddress(value));
  if (new Set(owners.map(owner => owner.toLowerCase())).size !== 3) throw new Error("Buyer, seller and arbitrator must have different addresses.");
  return owners;
}
export function safeInitializer(values: string[]): Hex {
  return encodeFunctionData({ abi: safeAbi, functionName: "setup", args: [safeOwners(values), 2n, zeroAddress, "0x", SAFE_FALLBACK, zeroAddress, 0n, zeroAddress] });
}
export interface SafePlan { chainId: SafeChainId; owners: Address[]; salt: string; address: Address; reference: Hex | null; state: "review" | "unknown" | "submitted" | "ready" | "failed" }
export function safeDeployment(values: string[], salt: bigint, creationCode: Hex) {
  if (salt < 0n || salt >= 2n ** 256n || creationCode === "0x") throw new Error("Invalid Safe deployment parameters.");
  const owners = safeOwners(values), initializer = safeInitializer(owners);
  const address = getCreate2Address({ from: SAFE_FACTORY, salt: keccak256(encodePacked(["bytes32", "uint256"], [keccak256(initializer), salt])), bytecodeHash: keccak256(concat([creationCode, pad(SAFE_SINGLETON, { size: 32 })])) });
  const data = encodeFunctionData({ abi: factoryAbi, functionName: "createProxyWithNonce", args: [SAFE_SINGLETON, initializer, salt] });
  return { owners, address, data };
}
export function safeAppUrl(address: string, chainId: SafeChainId): string {
  if (!isAddress(address) || address.toLowerCase() === zeroAddress) throw new Error("Enter a valid Safe address.");
  return `https://app.safe.global/home?safe=${safeNetwork(chainId).safePrefix}:${getAddress(address)}`;
}

const LEGACY_STORAGE_KEY = "pay3flow.otc.production-safe.v1";
export function safePlanStorageKey(chainId: SafeChainId): string {
  safeNetwork(chainId);
  return `${LEGACY_STORAGE_KEY}.${chainId}`;
}

function parseSafePlan(raw: string, chainId: SafeChainId, legacy = false): SafePlan {
  const saved = JSON.parse(raw);
  if (!saved || (saved.chainId === undefined && legacy ? 1 : saved.chainId) !== chainId) throw new Error("Saved Safe network differs from the selected network.");
  const owners = safeOwners(saved.owners);
  safeAppUrl(saved.address, chainId);
  if (typeof saved.salt !== "string" || !/^\d+$/.test(saved.salt) || BigInt(saved.salt) >= 2n ** 256n) throw new Error("Invalid saved Safe salt.");
  if (!["review", "unknown", "submitted", "ready", "failed"].includes(saved.state)
    || (saved.reference !== null && (typeof saved.reference !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(saved.reference)))
    || (saved.state === "submitted" && !saved.reference)) throw new Error("Invalid saved Safe status or transaction.");
  return { chainId, owners, salt: saved.salt, address: getAddress(saved.address), reference: saved.reference, state: saved.state };
}

type SafeStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;
export function loadSafePlan(storage: SafeStorage, chainId: SafeChainId): SafePlan | null {
  const key = safePlanStorageKey(chainId), raw = storage.getItem(key);
  if (raw !== null) return parseSafePlan(raw, chainId);
  if (chainId !== 1) return null;
  const legacy = storage.getItem(LEGACY_STORAGE_KEY);
  if (legacy === null) return null;
  const migrated = parseSafePlan(legacy, 1, true);
  // Persist before removing the legacy record, including unresolved sends.
  storeSafePlan(storage, migrated);
  storage.removeItem(LEGACY_STORAGE_KEY);
  return migrated;
}
export function storeSafePlan(storage: SafeStorage, plan: SafePlan): void {
  const raw = JSON.stringify(plan);
  parseSafePlan(raw, plan.chainId);
  storage.setItem(safePlanStorageKey(plan.chainId), raw);
}
