import { apiUrl, wsUrl } from "./api";
import type { DiscoveryQuery, DiscoveryResponse } from "./discovery";

/** Stream wildcard quotes into the normal Exchange results pipeline. */
export async function searchDiscovery(query: DiscoveryQuery, signal: AbortSignal, onUpdate: (response: DiscoveryResponse) => void): Promise<DiscoveryResponse> {
  let latest: DiscoveryResponse | undefined;
  try {
    return await new Promise<DiscoveryResponse>((resolve, reject) => {
      const socket = new WebSocket(wsUrl("/ws/p2p/discovery"));
      let finished = false;
      const finish = (error?: Error) => {
        if (finished) return;
        finished = true;
        clearTimeout(timer);
        signal.removeEventListener("abort", abort);
        socket.close();
        if (error) reject(error);
        else if (latest) resolve(latest);
        else reject(new Error("No search response received."));
      };
      const abort = () => finish(new DOMException("Search cancelled", "AbortError"));
      const timer = setTimeout(() => finish(new Error("Search timed out. Try again.")), 65_000);
      signal.addEventListener("abort", abort, { once: true });
      if (signal.aborted) return abort();
      socket.onopen = () => socket.send(JSON.stringify(query));
      socket.onmessage = event => {
        if (finished) return;
        try {
          const message = JSON.parse(event.data);
          if (message.type === "search_failed") return finish(new Error(message.error));
          if (message.response) { latest = message.response; onUpdate(latest!); }
          if (message.type === "search_finished") finish();
        } catch { finish(new Error("The search response could not be read.")); }
      };
      socket.onerror = () => finish(new Error("Live search connection failed."));
      socket.onclose = () => finish(new Error("Live search connection closed."));
    });
  } catch (cause) {
    // Never repeat fan-out after a partial result or an intentional cancellation.
    if (signal.aborted || latest) throw cause;
    const url = apiUrl("/api/p2p/discovery");
    for (const [key, value] of Object.entries(query)) if (value !== undefined) url.searchParams.set(key, String(value));
    const response = await fetch(url, { signal: AbortSignal.any([signal, AbortSignal.timeout(65_000)]) });
    if (!response.ok) throw new Error(await response.text());
    const result: DiscoveryResponse = await response.json();
    onUpdate(result);
    return result;
  }
}
