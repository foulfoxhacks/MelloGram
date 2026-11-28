import { copyFile, mkdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL("..", import.meta.url));
const output = new URL("../public/generated/", import.meta.url);
await mkdir(output, { recursive: true });
const lock = JSON.parse(await readFile(new URL("../package-lock.json", import.meta.url), "utf8"));
const notices = await Promise.all(Object.entries(lock.packages)
  .filter(([path, entry]) => path && !entry.dev)
  .map(async ([path, entry]) => {
    const license = path === "node_modules/qrcode-generator"
      ? "browser/licenses/qrcode-generator.txt"
      : `${path}/LICENSE`;
    const text = await readFile(new URL(`../${license}`, import.meta.url), "utf8");
    return `${path.slice("node_modules/".length)}@${entry.version}\n${text.trim()}`;
  }));
await build({
  absWorkingDir: root,
  entryPoints: { telegram: "browser/telegram.js" },
  outdir: fileURLToPath(output),
  bundle: true,
  format: "esm",
  platform: "browser",
  target: ["es2022"],
  minify: true,
  legalComments: "eof",
  banner: { js: `/*! Third-party licenses\n${notices.join("\n\n").replaceAll("*/", "* /")}\n*/` },
});
for (const name of ["mtcute.wasm", "mtcute-simd.wasm"]) {
  await copyFile(fileURLToPath(import.meta.resolve(`@mtcute/wasm/${name}`)), new URL(name, output));
}
