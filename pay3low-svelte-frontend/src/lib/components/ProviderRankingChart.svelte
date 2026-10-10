<script lang="ts">
  import type { ProfileDay } from '$lib/provider-profile';
  import { percent } from '$lib/provider-profile';
  import { locale } from '$lib/i18n';
  import { profileCopy } from '$lib/provider-profile-copy';
  export let days: ProfileDay[] = [];
  let active: number | null = null;
  $: copy = profileCopy($locale);
  $: activeDay = active === null ? null : days[active];
  $: observed = days.map((day, index) => ({ day, index })).filter(value => value.day.searches > 0);
  $: singleDay = observed.length === 1 ? observed[0] : null;
  $: tickIndices = singleDay ? [singleDay.index] : [...new Set([0, Math.floor((days.length - 1) / 4), Math.floor((days.length - 1) / 2), Math.floor((days.length - 1) * 3 / 4), days.length - 1])].filter(index => index >= 0);
  const left = 44, right = 780, top = 20, bottom = 220;
  $: line10 = singleDay ? '' : path(days, 'top10');
  $: line1 = singleDay ? '' : path(days, 'top1');
  function x(index: number) { return singleDay ? (left + right) / 2 : left + index / Math.max(1, days.length - 1) * (right - left); }
  function y(count: number, total: number) { return bottom - (percent(count, total) ?? 0) / 100 * (bottom - top); }
  function path(values: ProfileDay[], field: 'top10' | 'top1') {
    let started = false;
    return values.map((day, index) => {
      if (!day.searches) { started = false; return ''; }
      const segment = `${started ? 'L' : 'M'}${x(index).toFixed(2)},${y(day[field], day.searches).toFixed(2)}`;
      started = true;
      return segment;
    }).join(' ');
  }
  function date(value: string) { return new Date(value).toLocaleDateString($locale, { day: 'numeric', month: 'short', timeZone: 'UTC' }); }
</script>

<div class="chart" on:mouseleave={() => active = null} role="group" aria-label={copy.ranking}>
  <svg viewBox="0 0 800 260" role="img" aria-label={`${copy.ranking}. ${copy.timezone}`}>
    <title>{copy.ranking}</title>
    {#each [0, 25, 50, 75, 100] as value}
      <line class="grid" x1={left} x2={right} y1={bottom - value * 2} y2={bottom - value * 2} />
      <text class="axis" x="32" y={bottom - value * 2 + 4} text-anchor="end">{value}%</text>
    {/each}
    {#each tickIndices as index}
      <text class="axis" x={x(index)} y="252" text-anchor={singleDay ? 'middle' : index === 0 ? 'start' : index === days.length - 1 ? 'end' : 'middle'}>{date(days[index].started_at)}</text>
    {/each}
    {#if singleDay}
      {#each ['top10', 'top1'] as field, index}
        {@const count = singleDay.day[field as 'top10' | 'top1']}
        {@const barX = x(singleDay.index) + (index === 0 ? -72 : 14)}
        <rect class="dayBar" class:point10={field === 'top10'} class:point1={field === 'top1'} x={barX} y={y(count, singleDay.day.searches)} width="58" height={bottom - y(count, singleDay.day.searches)} rx="5" />
        <text class="barValue" x={barX + 29} y={Math.max(14, y(count, singleDay.day.searches) - 10)} text-anchor="middle">{Math.round(percent(count, singleDay.day.searches) ?? 0)}%</text>
      {/each}
    {:else}
    <path class="line10" d={line10} />
    <path class="line1" d={line1} />
    {#each days as day, index}
      {#if day.searches}
        <circle class="point10" cx={x(index)} cy={y(day.top10, day.searches)} r={active === index ? 5 : 2.5} />
        <circle class="point1" cx={x(index)} cy={y(day.top1, day.searches)} r={active === index ? 4 : 2} />
      {/if}
    {/each}
    {/if}
    {#if activeDay && active !== null}
      <line class="crosshair" x1={x(active)} x2={x(active)} y1={top} y2={bottom} />
    {/if}
  </svg>
  <div class="targets">
    {#each days as day, index}
      {#if day.searches}<button type="button" class="dayTarget" style:left={`${x(index) / 8}%`} style:width={`${singleDay ? 20 : 92 / Math.max(1, days.length)}%`} aria-label={`${date(day.started_at)}: ${copy.inTop10} ${Math.round(percent(day.top10, day.searches) ?? 0)}%, ${copy.participation} ${day.searches}`} on:mouseenter={() => active = index} on:focus={() => active = index} on:blur={() => active = null} on:click={() => active = active === index ? null : index}></button>{/if}
    {/each}
  </div>
  {#if activeDay && active !== null}
    <div class="tooltip" style:left={`${Math.max(15, Math.min(80, x(active) / 8))}%`} role="status">
      <strong>{date(activeDay.started_at)} · UTC</strong>
      <span><i class="lime"></i>{copy.inTop10}<b>{Math.round(percent(activeDay.top10, activeDay.searches) ?? 0)}%</b></span>
      <span><i class="purple"></i>{copy.inTop1}<b>{Math.round(percent(activeDay.top1, activeDay.searches) ?? 0)}%</b></span>
      <small>{copy.participation}: {activeDay.searches}</small>
    </div>
  {/if}
</div>
<div class="legend"><span><i class="lime"></i>{copy.inTop10}</span><span><i class="purple"></i>{copy.inTop1}</span><small>UTC</small></div>
{#if singleDay}<p class="singleDayNote">{copy.singleDayHint}</p>{/if}

<style>
  .chart { position: relative; width: 100%; margin-top: 30px; }
  svg { display: block; width: 100%; overflow: visible; }
  .grid { stroke: var(--color-border); stroke-dasharray: 3 5; }
  .axis { fill: var(--color-text-soft); font-family: var(--font-mono); font-size: 11px; }
  .barValue { fill: var(--color-text); font-family: var(--font-mono); font-size: 15px; font-weight: 700; }
  .singleDayNote { margin-top: 14px; color: var(--color-text-soft); font-size: 11px; }
  .line10, .line1 { fill: none; stroke-width: 3; stroke-linejoin: round; stroke-linecap: round; }
  .line10 { stroke: var(--color-accent-text); }.line1 { stroke: var(--color-violet); stroke-width: 2; }
  .point10 { fill: var(--color-accent-text); }.point1 { fill: var(--color-violet); }
  .crosshair { stroke: var(--color-text-faint); stroke-dasharray: 3 4; }
  .targets { position: absolute; inset: 8% 0 15%; pointer-events: none; }
  .dayTarget { position: absolute; height: 100%; min-width: 0; transform: translateX(-50%); pointer-events: auto; border-radius: 4px; }
  .dayTarget:focus-visible { outline-offset: 0; }
  .tooltip { position: absolute; top: 4%; width: 225px; transform: translateX(-50%); padding: 15px; pointer-events: none; border: 1px solid var(--color-border-strong); border-radius: 10px; background: var(--color-paper); box-shadow: 0 8px 30px #00000012; z-index: 3; font-size: 11px; }
  .tooltip strong { display: block; margin-bottom: 12px; }.tooltip span { display: flex; align-items: center; gap: 7px; margin-top: 8px; }.tooltip b { margin-left: auto; font-family: var(--font-mono); }.tooltip small { display: block; margin-top: 12px; color: var(--color-text-soft); }
  .legend { display: flex; align-items: center; gap: 24px; margin-top: 18px; color: var(--color-text-soft); font-size: 11px; }.legend span { display: flex; align-items: center; gap: 7px; }.legend small { margin-left: auto; font-family: var(--font-mono); }
  i { display: inline-block; width: 8px; height: 8px; border-radius: 50%; }.lime { background: var(--color-accent-text); }.purple { background: var(--color-violet); }
  @media (max-width: 600px) { .chart { margin-top: 24px; }.legend { gap: 16px; font-size: 10px; }svg { min-height: 160px; } }
</style>
