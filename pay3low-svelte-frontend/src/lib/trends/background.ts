import type { layoutWall } from "./model";

export type WallCell = ReturnType<typeof layoutWall>[number];
export type EmptySlot = { left: number; top: number; width: number; height: number };

/** Continue natural-width courses around occupied bricks; their gaps have no background tiles. */
export function emptyWallSlots(cells: WallCell[], widths: number[], left: number, right: number, top: number, bottom: number, height: number): EmptySlot[] {
  if (!widths.length) return [];
  const gap = 6;
  const pitch = height + 8;
  const cycle = widths.reduce((sum, width) => sum + width + gap, 0);
  const slots: EmptySlot[] = [];
  const rows = new Map<number, WallCell[]>();
  for (const cell of cells) {
    if (cell.row * pitch + height < top || cell.row * pitch > bottom) continue;
    const row = rows.get(cell.row) ?? [];
    row.push(cell);
    rows.set(cell.row, row);
  }
  const add = (x: number, y: number, width: number) => {
    if (width > 0 && x < right && x + width > left) slots.push({ left: x, top: y, width, height });
  };
  function repeat(anchor: number, start: number, end: number, y: number) {
    const first = Math.floor((Math.max(left, start) - anchor) / cycle);
    const last = Math.floor((Math.min(right, end) - anchor) / cycle);
    for (let index = first; index <= last; index++) {
      let x = anchor + index * cycle;
      for (const width of widths) {
        if (x >= start && x + width <= end) add(x, y, width);
        x += width + gap;
      }
    }
  }
  for (const [row, course] of rows) {
    course.sort((a, b) => a.left - b.left);
    const y = row * pitch;
    repeat(course[0].left, -Infinity, course[0].left - gap, y);
    for (let index = 1; index < course.length; index++) {
      let x = course[index - 1].left + course[index - 1].width + gap;
      const end = course[index].left - gap;
      let widthIndex = row + index;
      while (x < end) {
        const desired = widths[widthIndex++ % widths.length];
        const remaining = end - x;
        // Absorb a small remainder into this empty slot instead of leaving a partial hole.
        const width = remaining <= desired + gap + Math.min(...widths) ? remaining : desired;
        add(x, y, width);
        x += width + gap;
      }
    }
    const last = course.at(-1)!;
    repeat(last.left + last.width + gap, last.left + last.width + gap, Infinity, y);
  }
  return slots;
}
