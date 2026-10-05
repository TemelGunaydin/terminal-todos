import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants, readFileSync } from "node:fs";
import { access, chmod, lstat, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

export const APP_VERSION = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;
if (!/^\d+\.\d+\.\d+$/.test(APP_VERSION)) throw new Error("Invalid Terminal Todos release version");

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-musl",
  "linux-x64": "x86_64-unknown-linux-musl",
};
const MAX_BINARY_SIZE = 32 * 1024 * 1024;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function appSpec({ platform = process.platform, arch = process.arch, agentDir } = {}) {
  const target = TARGETS[`${platform}-${arch}`];
  if (!target) throw new Error(`Terminal Todos supports macOS/Linux on arm64 and x64, not ${platform}/${arch}.`);
  agentDir ??= process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");
  if (agentDir === "~" || agentDir.startsWith("~/")) agentDir = join(homedir(), agentDir.slice(2));
  const directory = join(resolve(agentDir), "tools", "terminal-todos", APP_VERSION, target);
  const asset = `terminal-todos-${APP_VERSION}-${target}`;
  return {
    version: APP_VERSION,
    target,
    asset,
    directory,
    binary: join(directory, "todo"),
    baseUrl: `https://github.com/TemelGunaydin/terminal-todos/releases/download/v${APP_VERSION}`,
  };
}

/** Only our verified installation is eligible; never probe an arbitrary PATH todo (it may be Swift). */
export async function installedApp(spec) {
  let directory;
  try {
    directory = await lstat(spec.directory);
  } catch (error) {
    if (error.code === "ENOENT") return undefined;
    throw error;
  }
  try {
    if (!directory.isDirectory() || directory.isSymbolicLink()) throw new Error("Invalid installation directory");
    const binary = await lstat(spec.binary);
    const receiptPath = join(spec.directory, "receipt.json");
    const receiptStat = await lstat(receiptPath);
    if (!binary.isFile() || binary.isSymbolicLink() || binary.size > MAX_BINARY_SIZE ||
        !receiptStat.isFile() || receiptStat.isSymbolicLink() || receiptStat.size > 2048) {
      throw new Error("Invalid installation files");
    }
    const receipt = JSON.parse(await readFile(receiptPath, "utf8"));
    if (receipt.version !== spec.version || receipt.target !== spec.target || !/^[a-f0-9]{64}$/.test(receipt.sha256)) {
      throw new Error("Invalid installation receipt");
    }
    await access(spec.binary, constants.X_OK);
    if (sha256(await readFile(spec.binary)) !== receipt.sha256) throw new Error("Binary checksum mismatch");
    return spec.binary;
  } catch (error) {
    throw new Error(`Terminal Todos installation failed validation. Remove only ${spec.directory} and retry /todo.`, { cause: error });
  }
}

async function download(url, limit, fetchImpl, signal) {
  const response = await fetchImpl(url, { signal: AbortSignal.any([signal, AbortSignal.timeout(120_000)]) });
  if (!response.ok) {
    if (response.status === 404) throw new Error(`Terminal Todos v${APP_VERSION} binary is not published for this platform yet. Retry after the release is published.`);
    throw new Error(`Terminal Todos download failed (HTTP ${response.status}).`);
  }
  if (!response.body) throw new Error("Empty Terminal Todos download");
  if (Number(response.headers.get("content-length")) > limit) {
    await response.body.cancel();
    throw new Error("Terminal Todos download exceeds the size limit");
  }
  const chunks = [];
  let size = 0;
  // Count streamed bytes too: Content-Length is not always present or trustworthy.
  for await (const chunk of response.body) {
    size += chunk.byteLength;
    if (size > limit) throw new Error("Terminal Todos download exceeds the size limit");
    chunks.push(chunk);
  }
  signal.throwIfAborted();
  return Buffer.concat(chunks);
}

function checkBinary(binary, probeHome) {
  const result = spawnSync(binary, ["--version"], {
    env: { ...process.env, HOME: probeHome, XDG_DATA_HOME: join(probeHome, "data") },
    encoding: "utf8",
    timeout: 5000,
    maxBuffer: 64 * 1024,
  });
  if (result.error || result.status !== 0 || result.stdout.trim() !== `todo ${APP_VERSION}`) {
    throw new Error("Downloaded Terminal Todos binary cannot run on this system or has the wrong version.", { cause: result.error });
  }
}

/** Call only after user approval. Stage beside the destination and publish a complete directory atomically. */
export async function installApp(spec, { signal = new AbortController().signal, fetchImpl = globalThis.fetch, verifyBinary = checkBinary } = {}) {
  signal.throwIfAborted();
  const existing = await installedApp(spec);
  if (existing) return existing;

  const checksum = (await download(`${spec.baseUrl}/${spec.asset}.sha256`, 2048, fetchImpl, signal)).toString("utf8");
  const match = /^([a-f0-9]{64})  ([^\r\n]+)\n?$/.exec(checksum);
  if (!match || match[2] !== spec.asset) throw new Error("Invalid Terminal Todos release checksum file");
  const bytes = await download(`${spec.baseUrl}/${spec.asset}`, MAX_BINARY_SIZE, fetchImpl, signal);
  if (sha256(bytes) !== match[1]) throw new Error("Terminal Todos SHA-256 mismatch; nothing was installed.");
  signal.throwIfAborted();

  const parent = dirname(spec.directory);
  await mkdir(parent, { recursive: true, mode: 0o700 });
  const stage = await mkdtemp(join(parent, ".install-"));
  try {
    const binary = join(stage, "todo");
    await writeFile(binary, bytes, { flag: "wx", mode: 0o700 });
    await chmod(binary, 0o700);
    const probeHome = join(stage, "probe-home");
    await mkdir(probeHome, { mode: 0o700 });
    await verifyBinary(binary, probeHome);
    await rm(probeHome, { recursive: true, force: true });
    signal.throwIfAborted();
    await writeFile(join(stage, "receipt.json"), JSON.stringify({ version: spec.version, target: spec.target, sha256: match[1] }) + "\n", { flag: "wx", mode: 0o600 });
    try {
      await rename(stage, spec.directory);
    } catch (error) {
      // Another approved Pi session may have installed the same release while we downloaded.
      if (error.code !== "EEXIST" && error.code !== "ENOTEMPTY") throw error;
      const winner = await installedApp(spec);
      if (!winner) throw error;
      return winner;
    }
    return spec.binary;
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
