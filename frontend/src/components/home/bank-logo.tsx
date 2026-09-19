import { PaymentMethod, paymentMethodFavicon } from "@/lib/payment-methods";

interface BankLogoProps {
  method: PaymentMethod | null;
  className: string;
  fallback: string;
}

export function BankLogo({ method, className, fallback }: BankLogoProps) {
  const favicon = paymentMethodFavicon(method);

  return (
    <span className={className} style={{ backgroundColor: method?.color ?? "#171a17" }} aria-hidden="true">
      {favicon && (
        <img
          src={favicon}
          alt=""
          onError={(event) => {
            event.currentTarget.hidden = true;
          }}
        />
      )}
      <span>{method?.initials ?? fallback}</span>
    </span>
  );
}
