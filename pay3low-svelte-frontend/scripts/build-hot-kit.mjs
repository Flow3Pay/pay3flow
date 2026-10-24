import { build } from "esbuild";
import stdlib from "node-stdlib-browser";
import polyfills from "node-stdlib-browser/helpers/esbuild/plugin";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { readFile, writeFile, mkdir, access } from "node:fs/promises";
const require = createRequire(import.meta.url);
const root = new URL("../",import.meta.url);
process.chdir(fileURLToPath(root));
const entry = "src/lib/hex/sdk-entry.ts", output = "static/vendor/hex-hot-kit.js", stamp = ".cache/hex-hot-kit.sha256";
const hash = createHash("sha256");
for (const path of ["pnpm-lock.yaml",entry,"scripts/build-hot-kit.mjs"]) hash.update(await readFile(path));
const digest = hash.digest("hex");
try { if (await readFile(stamp,"utf8") === digest) { await access(output); process.exit(0); } } catch { /* First build. */ }
await mkdir("static/vendor",{recursive:true});
await build({entryPoints:[entry],outfile:output,bundle:true,format:"esm",platform:"browser",target:"es2022",minify:true,
  plugins:[polyfills(stdlib)],inject:[require.resolve("node-stdlib-browser/helpers/esbuild/shim")],
  define:{global:"global",Buffer:"Buffer",process:"process","process.env.NODE_ENV":'"production"'},
  legalComments:"linked",logLevel:"warning"});
await mkdir(".cache",{recursive:true});
await writeFile(stamp,digest);
console.log("Built the local HOT wallet bundle");
