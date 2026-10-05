#!/usr/bin/env node
// Local preparation only. This script never publishes a release or updates Homebrew.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { APP_VERSION, appSpec } from "../extensions/managed-app.js";

const root = fileURLToPath(new URL("../", import.meta.url));
const spec = appSpec();
const target = process.argv[2];
if (target !== spec.target) throw new Error(`Build/package on the matching native runner: expected ${spec.target}, got ${target}`);
const cargo = await readFile(join(root, "Cargo.toml"), "utf8");
if (!cargo.includes(`version = "${APP_VERSION}"`)) throw new Error("Cargo and Pi package versions must match");
const binary = resolve(process.argv[3] || join(root, "target", target, "release", "todo"));
const home = await mkdtemp(join(tmpdir(), "terminal-todos-package-"));
try {
  const result = spawnSync(binary, ["--version"], {
    env: { ...process.env, HOME: home, XDG_DATA_HOME: join(home, "data") },
    encoding: "utf8", timeout: 5000,
  });
  if (result.error || result.status !== 0 || result.stdout.trim() !== `todo ${APP_VERSION}`) {
    throw new Error("Release binary has the wrong version or cannot run", { cause: result.error });
  }
  const dist = join(root, "dist");
  await mkdir(dist, { recursive: true });
  const output = join(dist, spec.asset);
  await copyFile(binary, output);
  const hash = createHash("sha256").update(await readFile(output)).digest("hex");
  await writeFile(`${output}.sha256`, `${hash}  ${spec.asset}\n`);
  console.log(`Prepared ${output}\nSHA-256: ${hash}\nNot published.`);
} finally {
  await rm(home, { recursive: true, force: true });
}
