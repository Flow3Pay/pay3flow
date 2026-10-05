// Local SVGs from lipis/flag-icons v7.3.2; license: static/icons/flags/LICENSE.
const flags: Record<string, string> = {
  AMD: "/icons/flags/am.svg",
  RUB: "/icons/flags/ru.svg",
  USD: "/icons/flags/us.svg",
  BYN: "/icons/flags/by.svg",
  UAH: "/icons/flags/ua.svg",
  KZT: "/icons/flags/kz.svg",
};

export function fiatFlagUrl(currency: string): string | null {
  return flags[currency.toUpperCase()] ?? null;
}
