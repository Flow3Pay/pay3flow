const round = (value: number) => Number(value.toFixed(1));

function verticalBoundary(x: number, rows: number, cell: number): string {
  const commands = [`M${x} 0`];
  for (let row = 0; row < rows; row += 1) {
    const start = row * cell;
    const end = start + cell;
    const center = start + cell * (0.42 + Math.random() * 0.16);
    const radius = cell * (0.12 + Math.random() * 0.045);
    const depth = cell * (0.16 + Math.random() * 0.075);
    const direction = Math.random() > 0.5 ? 1 : -1;
    commands.push(
      `V${round(center - radius)}`,
      `C${round(x + direction * depth)} ${round(center - radius)},${round(x + direction * depth)} ${round(center + radius)},${x} ${round(center + radius)}`,
      `V${end}`,
    );
  }
  return commands.join(" ");
}

function horizontalBoundary(y: number, columns: number, cell: number): string {
  const commands = [`M0 ${y}`];
  for (let column = 0; column < columns; column += 1) {
    const start = column * cell;
    const end = start + cell;
    const center = start + cell * (0.42 + Math.random() * 0.16);
    const radius = cell * (0.12 + Math.random() * 0.045);
    const depth = cell * (0.16 + Math.random() * 0.075);
    const direction = Math.random() > 0.5 ? 1 : -1;
    commands.push(
      `H${round(center - radius)}`,
      `C${round(center - radius)} ${round(y + direction * depth)},${round(center + radius)} ${round(y + direction * depth)},${round(center + radius)} ${y}`,
      `H${end}`,
    );
  }
  return commands.join(" ");
}

/** Builds one seamless, randomized jigsaw network for the page background. */
export function generatePuzzleBackground(): string {
  const columns = 6;
  const rows = 4;
  const cell = 100;
  const width = columns * cell;
  const height = rows * cell;
  const paths = [
    `M0 0V${height}M0 0H${width}`,
    ...Array.from({ length: columns - 1 }, (_, index) =>
      verticalBoundary((index + 1) * cell, rows, cell),
    ),
    ...Array.from({ length: rows - 1 }, (_, index) =>
      horizontalBoundary((index + 1) * cell, columns, cell),
    ),
  ];

  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">`,
    `<g fill="none" stroke="#161815" stroke-opacity=".105" stroke-width="1.45">`,
    ...paths.map((path) => `<path d="${path}"/>`),
    `</g>`,
    `</svg>`,
  ].join("");

  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}
