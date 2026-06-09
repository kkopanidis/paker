import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const rustCommandsPath = path.join(repoRoot, "src-tauri/src/commands/mod.rs");
const tauriTsPath = path.join(repoRoot, "src/lib/tauri.ts");

function parseRustCommands(source: string): string[] {
  const handlerBlock = source.match(
    /tauri::generate_handler!\[([\s\S]*?)\]/
  )?.[1];
  if (!handlerBlock) {
    throw new Error("Could not find tauri::generate_handler! block in mod.rs");
  }

  const commands = [...handlerBlock.matchAll(/::([a-z][a-z0-9_]*)\s*,/g)].map(
    (match) => match[1]
  );

  return [...new Set(commands)].sort();
}

function parseTsCommands(source: string): string[] {
  const commands = [
    ...source.matchAll(/invokeSafe(?:<[^>]*>)?\("([a-z][a-z0-9_]*)"/g),
  ].map((match) => match[1]);

  return [...new Set(commands)].sort();
}

describe("IPC contract between tauri.ts and Rust commands", () => {
  const rustCommands = parseRustCommands(fs.readFileSync(rustCommandsPath, "utf8"));
  const tsCommands = parseTsCommands(fs.readFileSync(tauriTsPath, "utf8"));

  it("registers every frontend invokeSafe command in Rust", () => {
    const missingInRust = tsCommands.filter((cmd) => !rustCommands.includes(cmd));
    expect(missingInRust, `Missing Rust handlers: ${missingInRust.join(", ")}`).toEqual(
      []
    );
  });

  it("documents Rust-only commands not referenced from tauri.ts", () => {
    const rustOnly = rustCommands.filter((cmd) => !tsCommands.includes(cmd));
    expect(rustOnly).toEqual([]);
  });
});
