// The upstream npm launcher does not consistently propagate forge's failure code.
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { spawn } from "node:child_process";
const require = createRequire(import.meta.url);
const [tool, ...args] = process.argv.slice(2);
if (!["forge", "anvil", "cast"].includes(tool)) throw new Error("Expected forge, anvil, or cast");
const platform = process.platform === "win32" ? "win32" : process.platform;
const arch = process.arch === "x64" ? "amd64" : process.arch;
const root = dirname(require.resolve(`@foundry-rs/${tool}-${platform}-${arch}/package.json`));
const executable = join(root, "bin", process.platform === "win32" ? `${tool}.exe` : tool);
const child = spawn(executable, args, { stdio: "inherit" });
child.on("error", error => { console.error(error.message); process.exitCode = 1; });
child.on("exit", (code, signal) => { process.exitCode = code ?? 1; if (signal) console.error(`Terminated by ${signal}`); });
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));
