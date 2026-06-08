#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const tauriConfPath = join(root, "src-tauri", "tauri.conf.json");

const rawTag = process.argv[2];
if (!rawTag) {
  console.error("Usage: node scripts/set-version-from-tag.mjs <vX.Y.Z>");
  process.exit(1);
}

const version = rawTag.replace(/^[vV]/, "");
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`Invalid release tag: ${rawTag} (expected vX.Y.Z)`);
  process.exit(1);
}

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
tauriConf.version = version;
writeFileSync(tauriConfPath, `${JSON.stringify(tauriConf, null, 2)}\n`);
console.log(`Set version ${version} -> src-tauri/tauri.conf.json`);

const sync = spawnSync("node", ["scripts/sync-version.mjs"], {
  cwd: root,
  stdio: "inherit",
});
if (sync.status !== 0) {
  process.exit(sync.status ?? 1);
}

console.log(`Version set from tag ${rawTag} (release version: ${version})`);
