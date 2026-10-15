import { mount } from "svelte";
import OtcProductionSafe from "../../src/lib/components/otc/OtcProductionSafe.svelte";

// Retained component coverage; the product UI does not mount this panel.
export function mountSafe() {
  const target = document.createElement("div");
  document.body.appendChild(target);
  mount(OtcProductionSafe, { target });
}
