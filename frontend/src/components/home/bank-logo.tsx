import { useState } from "react";

import { PaymentMethod, paymentMethodFavicon } from "@/lib/payment-methods";

interface BankLogoProps {
  method: PaymentMethod | null;
  className: string;
  fallback: string;
}

export function BankLogo({ method, className, fallback }: BankLogoProps) {
  const favicon = paymentMethodFavicon(method);
  const [failedFavicon, setFailedFavicon] = useState<string | null>(null);
  const showFavicon = Boolean(favicon && failedFavicon !== favicon);

  return (
    <span
      className={className}
      style={{ backgroundColor: showFavicon ? "transparent" : method?.color ?? "#171a17" }}
      aria-hidden="true"
    >
      {showFavicon && favicon && (
        <img
          src={favicon}
          alt=""
          onError={() => setFailedFavicon(favicon)}
        />
      )}
      {!showFavicon && <span>{method?.initials ?? fallback}</span>}
    </span>
  );
}
