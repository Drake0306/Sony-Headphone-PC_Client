// Fails when a colour literal appears outside the design tokens. Colours
// belong in src/styles/tokens.css; Headphone.svelte keeps its colourway tints.
import { readFileSync, readdirSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const allowed = join("src", "lib", "controls", "Headphone.svelte");
const literal = /#[0-9a-fA-F]{3,6}\b/;

function svelteFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return svelteFiles(path);
    return entry.name.endsWith(".svelte") ? [path] : [];
  });
}

const files = [...svelteFiles(join(root, "src")), join(root, "src", "styles", "base.css")];
const hits = [];
for (const file of files) {
  const rel = relative(root, file);
  if (rel === allowed) continue;
  readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, i) => {
      if (literal.test(line)) hits.push(`${rel.split(sep).join("/")}:${i + 1}: ${line.trim()}`);
    });
}

if (hits.length > 0) {
  console.error("Colour literals outside tokens.css:");
  for (const hit of hits) console.error(`  ${hit}`);
  process.exit(1);
}
