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

const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
const version = tauriConf.version;

if (!version || typeof version !== "string") {
  console.error("Could not read version from src-tauri/tauri.conf.json");
  process.exit(1);
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
