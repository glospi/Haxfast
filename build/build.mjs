// Page layer build (TS → a single IIFE to inject into the game page).
import { build, context } from "esbuild";
import { argv } from "node:process";

const watch = argv.includes("--watch");

const options = {
  entryPoints: ["page/src/index.ts"],
  bundle: true,
  format: "iife",
  target: "chrome146",
  outfile: "dist/page.js",
  sourcemap: "inline",
  logLevel: "info",
};

if (watch) {
  const ctx = await context(options);
  await ctx.watch();
  console.log("[build] watching page/…");
} else {
  await build(options);
}
