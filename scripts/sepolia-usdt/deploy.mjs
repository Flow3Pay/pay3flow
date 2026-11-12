import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import solc from "solc";
import { JsonRpcProvider, Wallet, ContractFactory, parseUnits, getAddress, formatUnits } from "ethers";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const RPC_URL = process.env.SEPOLIA_RPC_URL || "https://ethereum-sepolia-rpc.publicnode.com";
const RAW_KEY = (process.env.PRIVATE_KEY || "").trim();
const RAW_TO = (process.env.RECIPIENT || "0x5Cb4050a4686225cf5E2472400cDe43E868Ab45D").trim();
const SYMBOL = process.env.TOKEN_SYMBOL || "USDT";
const NAME = process.env.TOKEN_NAME || "Tether USD";
const WHOLE_TOKENS = process.env.WHOLE_TOKENS || "1000000000";

if (!/^0x[0-9a-fA-F]{64}$/.test(RAW_KEY)) {
  console.error("ERROR: set PRIVATE_KEY to a 32-byte hex key (0x...). Never commit it.");
  process.exit(1);
}

let recipient;
try {
  recipient = getAddress(RAW_TO.replace(/[\u0445\u0425]/g, "x"));
} catch (e) {
  console.error(`ERROR: recipient is not a valid checksummed address: ${RAW_TO}`);
  console.error("If the address was copied from a chat, mixed-case letters break the EIP-55 checksum.");
  console.error("Re-copy it from the wallet, or pass the all-lowercase form.");
  process.exit(1);
}

const source = fs.readFileSync(path.join(__dirname, "TestUSDT.sol"), "utf8");

function compile() {
  const input = {
    language: "Solidity",
    sources: { "TestUSDT.sol": { content: source } },
    settings: {
      optimizer: { enabled: true, runs: 200 },
      outputSelection: { "*": { "*": ["abi", "evm.bytecode.object"] } },
    },
  };
  const out = JSON.parse(solc.compile(JSON.stringify(input)));
  const errors = (out.errors || []).filter((e) => e.severity === "error");
  if (errors.length) {
    console.error(errors.map((e) => e.formattedMessage).join("\n"));
    process.exit(1);
  }
  const c = out.contracts["TestUSDT.sol"].TestUSDT;
  return { abi: c.abi, bytecode: "0x" + c.evm.bytecode.object };
}

const main = async () => {
  const provider = new JsonRpcProvider(RPC_URL);
  const wallet = new Wallet(RAW_KEY, provider);

  const net = await provider.getNetwork();
  if (net.chainId !== 11155111n) {
    console.error(`ERROR: connected to chainId ${net.chainId}, expected 11155111 (Sepolia).`);
    process.exit(1);
  }

  const balance = await provider.getBalance(wallet.address);
  console.log(`deployer : ${wallet.address}`);
  console.log(`balance  : ${formatUnits(balance, 18)} ETH`);
  console.log(`recipient: ${recipient}`);

  if (balance === 0n) {
    console.error("ERROR: deployer has 0 Sepolia ETH. Get gas at https://cloud.google.com/application/web3/faucet/ethereum/sepolia");
    process.exit(1);
  }

  const { abi, bytecode } = compile();
  const supply = parseUnits(WHOLE_TOKENS, 6);

  console.log(`\ndeploying TestUSDT (${NAME}, ${SYMBOL}, 6 decimals)...`);
  const factory = new ContractFactory(abi, bytecode, wallet);
  const contract = await factory.deploy(NAME, SYMBOL, supply, recipient);
  const tx = contract.deploymentTransaction();
  console.log(`tx       : ${tx.hash}`);
  await contract.waitForDeployment();
  const addr = await contract.getAddress();

  console.log(`token    : ${addr}`);
  console.log(`supply   : ${formatUnits(supply, 6)} ${SYMBOL} minted to ${recipient}`);
  console.log(`\nverify   : https://sepolia.etherscan.io/address/${addr}`);
  console.log(`tx       : https://sepolia.etherscan.io/tx/${tx.hash}`);
};

main().catch((e) => {
  console.error(e?.shortMessage || e?.message || e);
  process.exit(1);
});
