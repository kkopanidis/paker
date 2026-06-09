#!/usr/bin/env node
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const [version, hash] = process.argv.slice(2);

if (!version || !hash) {
  console.error("Usage: node scripts/render-scoop-manifest.mjs <version> <sha256-hex>");
  process.exit(1);
}

const manifest = {
  version,
  description: "Desktop browser for S3-compatible storage",
  homepage: "https://github.com/kkopanidis/paker",
  license: "MIT",
  url: `https://github.com/kkopanidis/paker/releases/download/v${version}/Paker-${version}-windows-portable.zip`,
  hash: `sha256:${hash.toLowerCase()}`,
  bin: "paker.exe",
  checkver: {
    github: "kkopanidis/paker",
  },
  autoupdate: {
    url: "https://github.com/kkopanidis/paker/releases/download/v$version/Paker-$version-windows-portable.zip",
  },
};

const outPath = join(dirname(fileURLToPath(import.meta.url)), "..", "scoop", "paker.json");
writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${outPath}`);
