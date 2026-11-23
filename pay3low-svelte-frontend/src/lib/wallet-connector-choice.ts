export interface WalletConnectorOption {
  id: string;
  name: string;
}

/** Stable within a detected-wallet list and independent of translated labels. */
export function walletConnectorId(name: string, index: number): string {
  return `${name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-") || "wallet"}:${index}`;
}

/** Select an explicit connector, or the only compatible connector. */
export function selectWalletConnector<T extends WalletConnectorOption>(
  choices: readonly T[],
  connectorId?: string | null,
): T | undefined {
  if (connectorId) return choices.find(choice => choice.id === connectorId);
  return choices.length === 1 ? choices[0] : undefined;
}
