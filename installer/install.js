#!/usr/bin/env node
/**
 * Dev Cockpit installer.
 *
 * Downloads the release build of Dev Cockpit.app from GitHub Releases,
 * verifies its SHA-256 checksum, extracts it (default: /Applications)
 * and launches it.
 *
 *   npx @minseokchae/dev-cockpit [--dir <path>] [--no-open]
 */

const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const APP_VERSION = "1.0.0";
const APP_SHA256 = "2f3fe62c87db9c69b37ff9f5492ebaa7f971597eea640b826c5e7e136554c434";
const ZIP_NAME = `Dev-Cockpit-${APP_VERSION}-macos-aarch64.zip`;
const URL = `https://github.com/Min0504/dev-cockpit/releases/download/v${APP_VERSION}/${ZIP_NAME}`;
const APP_NAME = "Dev Cockpit.app";

function fail(msg) {
  console.error(`\nerror: ${msg}`);
  process.exit(1);
}

function parseArgs(argv) {
  const args = { dir: null, open: true };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dir") {
      args.dir = argv[++i] || fail("--dir requires a path");
    } else if (a === "--no-open") {
      args.open = false;
    } else if (a === "--help" || a === "-h") {
      console.log(
        `Dev Cockpit installer v${APP_VERSION}\n\n` +
          `Usage: npx @minseokchae/dev-cockpit [options]\n\n` +
          `Options:\n` +
          `  --dir <path>   Install directory (default: /Applications, falls back to ~/Applications)\n` +
          `  --no-open      Do not launch the app after installing\n` +
          `  -h, --help     Show this help\n`
      );
      process.exit(0);
    } else {
      fail(`unknown option: ${a} (see --help)`);
    }
  }
  return args;
}

function pickInstallDir(preferred) {
  if (preferred) {
    fs.mkdirSync(preferred, { recursive: true });
    return preferred;
  }
  try {
    fs.accessSync("/Applications", fs.constants.W_OK);
    return "/Applications";
  } catch {
    const home = path.join(os.homedir(), "Applications");
    fs.mkdirSync(home, { recursive: true });
    console.log("note: /Applications is not writable, installing to ~/Applications");
    return home;
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (process.platform !== "darwin") {
    fail("Dev Cockpit is a macOS app.");
  }
  if (process.arch !== "arm64") {
    fail(
      "the prebuilt app is Apple Silicon (arm64) only.\n" +
        "On Intel Macs, build from source: https://github.com/Min0504/dev-cockpit#build--run"
    );
  }

  console.log(`Dev Cockpit ${APP_VERSION}`);
  console.log(`downloading ${URL} ...`);
  const res = await fetch(URL);
  if (!res.ok) {
    fail(`download failed: HTTP ${res.status} ${res.statusText}`);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  console.log(`downloaded ${(buf.length / 1024 / 1024).toFixed(1)} MB`);

  const sha = crypto.createHash("sha256").update(buf).digest("hex");
  if (sha !== APP_SHA256) {
    fail(`checksum mismatch — refusing to install.\n  expected ${APP_SHA256}\n  got      ${sha}`);
  }
  console.log("checksum verified (sha256)");

  const tmpZip = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "dev-cockpit-")), ZIP_NAME);
  fs.writeFileSync(tmpZip, buf);

  const installDir = pickInstallDir(args.dir);
  const appPath = path.join(installDir, APP_NAME);

  if (fs.existsSync(appPath)) {
    console.log(`replacing existing ${appPath}`);
    fs.rmSync(appPath, { recursive: true, force: true });
  }

  // ditto preserves bundle structure, permissions and extended attributes.
  const extract = spawnSync("ditto", ["-x", "-k", tmpZip, installDir], { stdio: "inherit" });
  if (extract.status !== 0) {
    fail("extraction failed (ditto)");
  }
  fs.rmSync(path.dirname(tmpZip), { recursive: true, force: true });

  // The app is not code-signed; strip the quarantine flag so Gatekeeper
  // does not block the first launch. Harmless if the attribute is absent.
  spawnSync("xattr", ["-dr", "com.apple.quarantine", appPath], { stdio: "ignore" });

  console.log(`installed: ${appPath}`);

  if (args.open) {
    const open = spawnSync("open", [appPath], { stdio: "ignore" });
    if (open.status === 0) {
      console.log("launched — look for the gauge icon in your menu bar (toggle with ⌃⌥D)");
    } else {
      console.log(`launch it with: open "${appPath}"`);
    }
  } else {
    console.log(`launch it with: open "${appPath}"`);
  }
}

main().catch((e) => fail(e && e.message ? e.message : String(e)));
