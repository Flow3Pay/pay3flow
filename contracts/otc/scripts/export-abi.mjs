import { readFile, writeFile, mkdir } from "node:fs/promises";
for (const [source, name] of [["OtcEscrowFactory.sol", "OtcEscrowFactory"], ["OtcEscrow.sol", "OtcEscrow"], ["Safe.sol", "Safe"]]) {
  const artifact = JSON.parse(await readFile(new URL(`../out/${source}/${name}.json`, import.meta.url), "utf8"));
  const directory = new URL("../abi/", import.meta.url);
  await mkdir(directory, { recursive: true });
  await writeFile(new URL(`${name}.json`, directory), JSON.stringify(artifact.abi, null, 2) + "\n");
}
