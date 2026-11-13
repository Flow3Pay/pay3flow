export interface RefreshCounter {
  /** Raw age of the latest completed snapshot. */
  secondsSinceUpdate: number | null;
  /** Age shown in the market bar, capped by the auto-refresh interval. */
  elapsedSeconds: number | null;
  secondsUntilRefresh: number | null;
}

/**
 * Keep the age readout steady while a search is in flight. Auto-refresh starts
 * at the displayed interval, so the counter must not keep growing until the
 * slower route search completes.
 */
export function refreshCounter(
  now: number,
  lastUpdatedAt: number | null,
  refreshSeconds: number,
  searchStartedAt: number | null,
  searching: boolean,
): RefreshCounter {
  const reference = searching && searchStartedAt !== null ? searchStartedAt : now;
  const secondsSinceUpdate = lastUpdatedAt === null ? null : Math.max(0, Math.floor((reference - lastUpdatedAt) / 1000));
  const elapsedSeconds = secondsSinceUpdate !== null && refreshSeconds
    ? Math.min(secondsSinceUpdate, refreshSeconds)
    : secondsSinceUpdate;
  const secondsUntilRefresh = refreshSeconds && elapsedSeconds !== null ? refreshSeconds - elapsedSeconds : null;
  return { secondsSinceUpdate, elapsedSeconds, secondsUntilRefresh };
}
