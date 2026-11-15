export interface MobileWalletLink {
  name: string;
  href: string;
}

export function mobileWalletLinks(pageUrl: string): MobileWalletLink[] {
  const page = new URL(pageUrl);
  const dappPath = `${page.host}${page.pathname}${page.search}${page.hash}`;
  return [
    { name: "MetaMask", href: `https://metamask.app.link/dapp/${dappPath}` },
    { name: "Trust Wallet", href: `https://link.trustwallet.com/open_url?coin_id=60&url=${encodeURIComponent(page.href)}` },
    { name: "Coinbase Wallet", href: `https://go.cb-w.com/dapp?cb_url=${encodeURIComponent(page.href)}` },
  ];
}
