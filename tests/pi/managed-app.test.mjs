import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { APP_VERSION, appSpec, installedApp, installApp } from "../../extensions/managed-app.js";

const bytes = Buffer.from("fixture executable, not a real app");
const hash = createHash("sha256").update(bytes).digest("hex");
async function sandbox(t) {
  const root = await mkdtemp(join(tmpdir(), "terminal-todos-install-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  return appSpec({ platform: "darwin", arch: "arm64", agentDir: root });
}
function transport(spec, binary = bytes, checksum = `${hash}  ${spec.asset}\n`) {
  const urls = [];
  return {
    urls,
    fetchImpl: async (url) => {
      urls.push(url);
      assert.ok(url.startsWith(`${spec.baseUrl}/`));
      return new Response(url.endsWith(".sha256") ? checksum : binary);
    },
    verifyBinary: async (path, home) => {
      assert.deepEqual(await readFile(path), bytes);
      assert.ok((await lstat(home)).isDirectory());
      assert.notEqual(home, process.env.HOME);
    },
  };
}

test("manifest, Cargo and lock versions agree; package exposes only the entrypoint", async () => {
  const pkg = JSON.parse(await readFile(new URL("../../package.json", import.meta.url), "utf8"));
  assert.equal(pkg.version, APP_VERSION);
  assert.deepEqual(pkg.pi.extensions, ["./extensions/todo.js"]);
  assert.equal(pkg.peerDependencies["@earendil-works/pi-coding-agent"], "*");
  assert.ok((await readFile(new URL("../../Cargo.toml", import.meta.url), "utf8")).includes(`version = "${APP_VERSION}"`));
  assert.ok((await readFile(new URL("../../Cargo.lock", import.meta.url), "utf8")).includes(`name = "terminal-todos"\nversion = "${APP_VERSION}"`));
});

test("four platform/CPU targets are pinned, unsupported platforms fail before I/O", () => {
  for (const [platform, arch, target] of [
    ["darwin", "arm64", "aarch64-apple-darwin"], ["darwin", "x64", "x86_64-apple-darwin"],
    ["linux", "arm64", "aarch64-unknown-linux-musl"], ["linux", "x64", "x86_64-unknown-linux-musl"],
  ]) {
    const spec = appSpec({ platform, arch, agentDir: "/tmp/pi space" });
    assert.equal(spec.target, target);
    assert.equal(spec.asset, `terminal-todos-${APP_VERSION}-${target}`);
    assert.equal(spec.baseUrl, `https://github.com/TemelGunaydin/terminal-todos/releases/download/v${APP_VERSION}`);
  }
  assert.throws(() => appSpec({ platform: "win32", arch: "x64" }), /supports macOS\/Linux/);
  assert.throws(() => appSpec({ platform: "linux", arch: "ia32" }), /not linux\/ia32/);
});

test("missing app detection neither creates files nor runs a PATH executable", async (t) => {
  const spec = await sandbox(t);
  assert.equal(await installedApp(spec), undefined);
  assert.deepEqual(await readdir(spec.directory.split("/tools/")[0]), []);
});

test("verified download is committed with a receipt; later calls are offline and idempotent", async (t) => {
  const spec = await sandbox(t);
  const options = transport(spec);
  assert.equal(await installApp(spec, options), spec.binary);
  assert.equal(await installedApp(spec), spec.binary);
  assert.deepEqual(options.urls, [`${spec.baseUrl}/${spec.asset}.sha256`, `${spec.baseUrl}/${spec.asset}`]);
  assert.equal((await lstat(spec.binary)).mode & 0o777, 0o700);
  const receipt = JSON.parse(await readFile(join(spec.directory, "receipt.json"), "utf8"));
  assert.deepEqual(receipt, { version: APP_VERSION, target: spec.target, sha256: hash });
  assert.deepEqual(await readdir(dirname(spec.directory)), [spec.target]);
  assert.equal(await installApp(spec, { fetchImpl: () => assert.fail("cache hit must not fetch") }), spec.binary);
});

test("checksum mismatch never stages or runs the unverified binary", async (t) => {
  const spec = await sandbox(t);
  await assert.rejects(installApp(spec, {
    ...transport(spec, Buffer.from("tampered")), verifyBinary: () => assert.fail("unverified code must not execute"),
  }), /SHA-256 mismatch/);
  assert.equal(await installedApp(spec), undefined);
});

test("malformed or wrong-filename checksum fails before binary download", async (t) => {
  for (const checksum of ["garbage", `${hash}  another-app\n`, `${hash}  ../../todo\n`]) {
    const spec = await sandbox(t);
    const options = transport(spec, bytes, checksum);
    await assert.rejects(installApp(spec, options), /Invalid.*checksum/);
    assert.equal(options.urls.length, 1);
    assert.equal(await installedApp(spec), undefined);
  }
});

test("not-yet-published, server and network failures leave no installation", async (t) => {
  for (const status of [404, 500]) {
    const spec = await sandbox(t);
    await assert.rejects(installApp(spec, { fetchImpl: async () => new Response("", { status }) }),
      status === 404 ? /not published/ : /HTTP 500/);
    assert.equal(await installedApp(spec), undefined);
  }
  const spec = await sandbox(t);
  await assert.rejects(installApp(spec, { fetchImpl: async () => { throw new Error("offline"); } }), /offline/);
});

test("oversized downloads are rejected with and without Content-Length", async (t) => {
  for (const headers of [{ "content-length": "2049" }, {}]) {
    const spec = await sandbox(t);
    await assert.rejects(installApp(spec, { fetchImpl: async () => new Response("x".repeat(2049), { headers }) }), /size limit/);
    assert.equal(await installedApp(spec), undefined);
  }
});

test("oversized binary headers and streams are rejected before validation or staging", async (t) => {
  for (const streamed of [false, true]) {
    const spec = await sandbox(t);
    await assert.rejects(installApp(spec, {
      fetchImpl: async (url) => {
        if (url.endsWith(".sha256")) return new Response(`${hash}  ${spec.asset}\n`);
        if (!streamed) return new Response("x", { headers: { "content-length": String(32 * 1024 * 1024 + 1) } });
        return new Response(new ReadableStream({ start(controller) {
          controller.enqueue(new Uint8Array(32 * 1024 * 1024));
          controller.enqueue(new Uint8Array(1));
          controller.close();
        } }));
      },
      verifyBinary: () => assert.fail("oversized binary must not execute"),
    }), /size limit/);
    assert.equal(await installedApp(spec), undefined);
  }
});

test("pre-cancelled and in-flight installs do not produce a runnable cache", async (t) => {
  const spec = await sandbox(t);
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(installApp(spec, { signal: controller.signal, fetchImpl: () => assert.fail("no request after cancellation") }), { name: "AbortError" });
  const active = new AbortController();
  const pending = installApp(spec, {
    signal: active.signal,
    fetchImpl: async (_url, { signal }) => {
      active.abort();
      signal.throwIfAborted();
    },
  });
  await assert.rejects(pending, { name: "AbortError" });
  assert.equal(await installedApp(spec), undefined);
});

test("incompatible binary or cancellation during validation cleans staging", async (t) => {
  const spec = await sandbox(t);
  await assert.rejects(installApp(spec, {
    ...transport(spec), verifyBinary: async () => { throw new Error("wrong CPU"); },
  }), /wrong CPU/);
  assert.deepEqual(await readdir(dirname(spec.directory)), []);
  const controller = new AbortController();
  await assert.rejects(installApp(spec, {
    ...transport(spec), signal: controller.signal, verifyBinary: async () => controller.abort(),
  }), { name: "AbortError" });
  assert.deepEqual(await readdir(dirname(spec.directory)), []);
});

test("corrupt, stale, non-executable or symlinked caches fail closed without executing", async (t) => {
  for (const kind of ["bytes", "version", "target", "permissions", "symlink", "directory"]) {
    const spec = await sandbox(t);
    await installApp(spec, transport(spec));
    if (kind === "bytes") await writeFile(spec.binary, "changed");
    if (kind === "version" || kind === "target") {
      const receipt = { version: APP_VERSION, target: spec.target, sha256: hash, [kind]: "old" };
      await writeFile(join(spec.directory, "receipt.json"), JSON.stringify(receipt));
    }
    if (kind === "permissions") await chmod(spec.binary, 0o600);
    if (kind === "symlink") {
      await rm(spec.binary);
      await symlink("receipt.json", spec.binary);
    }
    if (kind === "directory") {
      await rm(spec.binary);
      await mkdir(spec.binary);
    }
    await assert.rejects(installedApp(spec), /failed validation/);
    await assert.rejects(installApp(spec, { fetchImpl: () => assert.fail("must not overwrite invalid cache") }), /failed validation/);
  }
});

test("concurrent approved installs converge without partial directories", async (t) => {
  const spec = await sandbox(t);
  const results = await Promise.all([installApp(spec, transport(spec)), installApp(spec, transport(spec))]);
  assert.deepEqual(results, [spec.binary, spec.binary]);
  assert.equal(await installedApp(spec), spec.binary);
  assert.deepEqual(await readdir(dirname(spec.directory)), [spec.target]);
});
