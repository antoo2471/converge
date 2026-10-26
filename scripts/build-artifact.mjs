import { build } from "esbuild";
import { mkdir, writeFile } from "node:fs/promises";
await mkdir("src/generated", { recursive: true });
const result = await build({
  entryPoints: ["src/artifact-runtime.tsx"],
  bundle: true,
  write: false,
  minify: true,
  format: "iife",
  define: { "process.env.NODE_ENV": '"production"' },
  target: "es2022",
});
await writeFile(
  "src/generated/artifact-runtime.txt",
  result.outputFiles[0].text,
);
