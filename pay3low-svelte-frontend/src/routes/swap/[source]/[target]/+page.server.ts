import { error } from "@sveltejs/kit";
import { shareAmount, shareCurrency } from "$lib/share";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = ({ params, url }) => {
  const source = shareCurrency(params.source);
  const target = shareCurrency(params.target);
  if (!source || !target) error(404, "Unknown exchange pair");
  const amount = shareAmount(url.searchParams.get("amount"));
  const receive = shareAmount(url.searchParams.get("receive"));
  const image = new URL("/share-image.png", url.origin);
  image.searchParams.set("from", source);
  image.searchParams.set("to", target);
  if (amount) image.searchParams.set("amount", amount);
  if (receive) image.searchParams.set("receive", receive);
  return { source, target, amount, receive, image: image.toString(), canonical: url.toString() };
};
