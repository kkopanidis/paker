#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

const tauriConfPath = join(root, "src-tauri", "tauri.conf.json");
const packageJsonPath = join(root, "package.json");
const packageLockPath = join(root, "package-lock.json");
const cargoTomlPath = join(root, "src-tauri", "Cargo.toml");

const checkOnly = process.argv.includes("--check");

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
const version = tauriConf.version;

if (!version || typeof version !== "string") {
  console.error("Could not read version from src-tauri/tauri.conf.json");
  process.exit(1);
}

if (checkOnly) {
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  const packageLock = JSON.parse(readFileSync(packageLockPath, "utf8"));
  const cargoToml = readFileSync(cargoTomlPath, "utf8");
  const cargoMatch = cargoToml.match(/^version = "(.*)"$/m);
  const cargoVersion = cargoMatch?.[1];

  const mismatches = [];
  if (packageJson.version !== version) {
    mismatches.push(`package.json (${packageJson.version})`);
  }
  if (packageLock.version !== version) {
    mismatches.push(`package-lock.json (${packageLock.version})`);
  }
  if (packageLock.packages?.[""]?.version !== version) {
    mismatches.push(`package-lock.json root package (${packageLock.packages?.[""]?.version})`);
  }
  if (cargoVersion !== version) {
    mismatches.push(`Cargo.toml (${cargoVersion})`);
  }

  if (mismatches.length > 0) {
    console.error(`Version mismatch: canonical is ${version} in tauri.conf.json`);
    for (const item of mismatches) {
      console.error(`  - ${item}`);
    }
    console.error("Run: npm run version:sync");
    process.exit(1);
  }

  console.log(`Version check passed (${version})`);
  process.exit(0);
}

const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
packageJson.version = version;
writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);
console.log(`Synced version ${version} -> package.json`);

const packageLock = JSON.parse(readFileSync(packageLockPath, "utf8"));
packageLock.version = version;
if (packageLock.packages?.[""]) {
  packageLock.packages[""].version = version;
}
writeFileSync(packageLockPath, `${JSON.stringify(packageLock, null, 2)}\n`);
console.log(`Synced version ${version} -> package-lock.json`);

const cargoToml = readFileSync(cargoTomlPath, "utf8");
const updatedCargoToml = cargoToml.replace(
  /^version = ".*"$/m,
  `version = "${version}"`,
);
writeFileSync(cargoTomlPath, updatedCargoToml);
console.log(`Synced version ${version} -> src-tauri/Cargo.toml`);

console.log(`Version sync complete (canonical: src-tauri/tauri.conf.json -> ${version})`);
