import { type TypedDataDefinition, type TypedData } from "viem";
import { connectWallet } from "$lib/wallet-session";
import { request, type Trade, type Session } from "./service";

export * from "./escrow-model";
import { validateEscrowTypedData, type Escrow, type EscrowCall, type EscrowProposal } from "./escrow-model";

async function evmWallet(expected: string) {
  const wallet = await connectWallet("ethereum");
  if (wallet.family !== "evm" || wallet.chainId !== 1 || wallet.address.toLowerCase() !== expected.toLowerCase()) throw new Error("Connect the booked owner wallet on Ethereum mainnet.");
  const { getWalletClient } = await import("@wagmi/core");
  const client = await getWalletClient(wallet.config as Parameters<typeof getWalletClient>[0], { chainId: 1 });
  return { wallet, client };
}

export async function signEscrow(session: Session, trade: Trade, escrow: Escrow, owner: string, proposal?: EscrowProposal): Promise<void> {
  const data = proposal?.typed_data ?? escrow.typed_data;
  validateEscrowTypedData(trade, escrow, data, proposal);
  const { wallet, client } = await evmWallet(owner);
  const message = Object.fromEntries(Object.entries(data.message).map(([name, value]) => {
    const field = data.types[data.primaryType]?.find(field => field.name === name);
    return [name, field?.type.startsWith("uint") ? BigInt(value) : value];
  }));
  const signature = await client.signTypedData({ ...data, message, account: wallet.address } as unknown as TypedDataDefinition<TypedData> & { account: `0x${string}` });
  await request(`/trades/${trade.id}/escrow/${proposal ? "approvals" : "consent"}`, session.token,
    proposal ? { proposal_id: proposal.id, signature } : { signature }, crypto.randomUUID());
}

/** Called only after an explicit review, never on component mount or session restoration. */
export async function sendEscrowCall(session: Session, tradeId: string, call: EscrowCall): Promise<void> {
  const instruction = call.instructions;
  if (instruction.chain_id !== 1 || BigInt(instruction.value) !== 0n) throw new Error("Invalid escrow handoff.");
  const { wallet } = await evmWallet(instruction.from);
  const nonce = BigInt(instruction.nonce);
  if (nonce > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("Wallet nonce cannot be represented safely.");
  const { sendTransaction } = await import("@wagmi/core");
  const reference = await sendTransaction(wallet.config as Parameters<typeof sendTransaction>[0], {
    chainId: 1, account: wallet.address, to: instruction.to as `0x${string}`, data: instruction.data as `0x${string}`, value: 0n, nonce: Number(nonce),
  });
  sessionStorage.setItem(`pay3flow.otc.escrow.reference.${call.id}`, reference);
  await request(`/trades/${tradeId}/escrow/reference`, session.token, { call_id: call.id, reference }, crypto.randomUUID());
}
