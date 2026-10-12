// Entirely local: Anvil contracts + Rust/PostgreSQL OTC service. EVER receipts are fixtures.
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:net";
import { createPublicClient, createWalletClient, http, keccak256 } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { mainnet } from "viem/chains";
import { fileURLToPath } from "node:url";

if (!process.env.OTC_TEST_DATABASE_URL) throw new Error("OTC_TEST_DATABASE_URL must point to a disposable database");
const directory = fileURLToPath(new URL("../", import.meta.url));
const server = createServer();
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;
await new Promise(resolve => server.close(resolve));
const rpc = `http://127.0.0.1:${port}`;
const anvil = spawn(process.execPath, ["scripts/foundry.mjs", "anvil", "--chain-id", "1", "--slots-in-an-epoch", "1", "--port", String(port), "--silent"], { cwd: directory, stdio: ["ignore", "inherit", "inherit"] });
const publicClient = createPublicClient({ chain: mainnet, transport: http(rpc) });
const account = privateKeyToAccount(`0x${"b".padStart(64, "0")}`);
const wallet = createWalletClient({ account, chain: mainnet, transport: http(rpc) });
const artifacts = async (source, name) => JSON.parse(await readFile(new URL(`../out/${source}/${name}.json`, import.meta.url), "utf8"));
const deploy = async (artifact, args = []) => {
  const hash = await wallet.deployContract({ abi: artifact.abi, bytecode: artifact.bytecode.object, args });
  return (await publicClient.waitForTransactionReceipt({ hash })).contractAddress;
};
try {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    try { await publicClient.getChainId(); ready = true; break; } catch { await new Promise(resolve => setTimeout(resolve, 50)); }
  }
  if (!ready) throw new Error("Local Anvil did not start");
  await publicClient.request({ method: "anvil_setBalance", params: [account.address, "0x3635c9adc5dea00000"] });
  const singleton = await deploy(await artifacts("Safe.sol", "Safe"));
  const mock = await deploy(await artifacts("OtcEscrow.t.sol", "MockUsdt"));
  const usdt = "0xdac17f958d2ee523a2206206994597c13d831ec7";
  await publicClient.request({ method: "anvil_setCode", params: [usdt, await publicClient.getCode({ address: mock })] });
  const arbitrator = privateKeyToAccount(`0x${"21".padStart(64, "0")}`).address;
  const factory = await deploy(await artifacts("OtcEscrowFactory.sol", "OtcEscrowFactory"), [usdt, arbitrator, singleton]);
  await publicClient.request({method:"anvil_mine",params:["0x4"]});
  console.log("Local chain checkpoints:", (await publicClient.getBlock({blockTag:"latest"})).number.toString(), (await publicClient.getBlock({blockTag:"finalized"})).number.toString());
  const env = { ...process.env, ESCROW_TEST_RPC_URL: rpc, ESCROW_TEST_FACTORY: factory.toLowerCase(), ESCROW_TEST_FACTORY_CODE_HASH: keccak256(await publicClient.getCode({address:factory})), ESCROW_TEST_SINGLETON:singleton.toLowerCase(), ESCROW_TEST_SINGLETON_CODE_HASH:keccak256(await publicClient.getCode({address:singleton})), ESCROW_TEST_ARBITRATOR:arbitrator.toLowerCase() };
  const cargo = process.env.CARGO_BIN || "cargo";
  const child = spawn(cargo, ["test", "--manifest-path", "../../backend/Cargo.toml", "--lib", "otc::escrow::tests::local_chain_roundtrip", "--", "--ignored", "--nocapture", "--test-threads=1"], { cwd: directory, env, stdio: "inherit" });
  const code = await new Promise((resolve, reject) => { child.on("error", reject); child.on("exit", code => resolve(code ?? 1)); });
  process.exitCode = code;
} finally { anvil.kill("SIGTERM"); }
