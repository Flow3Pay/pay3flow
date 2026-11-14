import { readFile } from "node:fs/promises";
import { privateKeyToAccount } from "viem/accounts";
import { createPublicClient, createWalletClient, defineChain, http, parseUnits } from "viem";

const TESTNETS = {
  sepolia: { chainId: 11155111, rpcEnv: "SEPOLIA_RPC_URL" },
  optimism_sepolia: { chainId: 11155420, rpcEnv: "OPTIMISM_SEPOLIA_RPC_URL" },
  bnb_testnet: { chainId: 97, rpcEnv: "BNB_TESTNET_RPC_URL" },
  gnosis_chiado: { chainId: 10200, rpcEnv: "GNOSIS_CHIADO_RPC_URL" },
  polygon_amoy: { chainId: 80002, rpcEnv: "POLYGON_AMOY_RPC_URL" },
  avalanche_fuji: { chainId: 43113, rpcEnv: "AVALANCHE_FUJI_RPC_URL" },
  arbitrum_sepolia: { chainId: 421614, rpcEnv: "ARBITRUM_SEPOLIA_RPC_URL" },
  base_sepolia: { chainId: 84532, rpcEnv: "BASE_SEPOLIA_RPC_URL" },
  sei_atlantic_2: { chainId: 1328, rpcEnv: "SEI_ATLANTIC_2_RPC_URL" },
};

function required(name) {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function parseNetworks(raw) {
  const names = raw.split(",").map(value => value.trim()).filter(Boolean);
  if (!names.length) throw new Error("TESTNETS must name at least one supported EVM testnet");
  for (const name of names) {
    if (!Object.hasOwn(TESTNETS, name)) throw new Error(`Unsupported testnet: ${name}`);
  }
  return [...new Set(names)];
}

const key = required("TESTNET_PRIVATE_KEY");
if (!/^0x[0-9a-fA-F]{64}$/.test(key)) throw new Error("TESTNET_PRIVATE_KEY must be a 32-byte hex key");
const account = privateKeyToAccount(key);
const holder = (process.env.TEST_TOKEN_HOLDER || account.address).trim();
if (!/^0x[0-9a-fA-F]{40}$/.test(holder) || /^0x0{40}$/i.test(holder)) {
  throw new Error("TEST_TOKEN_HOLDER must be a nonzero EVM address");
}
const networks = parseNetworks(required("TESTNETS"));
const wholeSupply = process.env.TEST_TOKEN_SUPPLY || "1000000";
const supply = parseUnits(wholeSupply, 6);
const artifact = JSON.parse(await readFile(new URL("../out/TestStablecoin.sol/TestStablecoin.json", import.meta.url), "utf8"));

for (const name of networks) {
  const settings = TESTNETS[name];
  const rpcUrl = required(settings.rpcEnv);
  const chain = defineChain({
    id: settings.chainId,
    name,
    nativeCurrency: { name: "Test gas", symbol: "GAS", decimals: 18 },
    rpcUrls: { default: { http: [rpcUrl] } },
  });
  const transport = http(rpcUrl);
  const publicClient = createPublicClient({ chain, transport });
  const walletClient = createWalletClient({ account, chain, transport });
  const actualChainId = await publicClient.getChainId();
  if (actualChainId !== settings.chainId) {
    throw new Error(`${name} RPC returned chain ID ${actualChainId}; expected ${settings.chainId}`);
  }

  console.log(`${name} (${settings.chainId}): deploying faucet USDT and USDC for ${holder}`);
  for (const [symbol, tokenName] of [["USDT", "Test Tether"], ["USDC", "Test USD Coin"]]) {
    const hash = await walletClient.deployContract({
      abi: artifact.abi,
      bytecode: artifact.bytecode.object,
      args: [tokenName, symbol, 6, holder, supply],
    });
    const receipt = await publicClient.waitForTransactionReceipt({ hash });
    if (receipt.status !== "success" || !receipt.contractAddress) {
      throw new Error(`${name} ${symbol} deployment failed: ${hash}`);
    }
    console.log(`${symbol}: ${receipt.contractAddress} (tx ${hash})`);
  }
}
