#!/usr/bin/env node
// Generates the Tauri updater manifest (`latest.json`) from a signed `tauri build` output
// (0.3.2 PR2, #69; `03 §11b`). The updater plugin fetches this file from GitHub Releases,
// verifies the minisign signature it carries against the pubkey baked into tauri.conf.json,
// then downloads + verifies the installer at `url`. Used by both the release workflow
// (.github/workflows/release.yml) and the documented local-release fallback + the PR2 E2E
// runbook (docs/TESTING.md) — one tested code path.
//
// Usage:
//   node scripts/make-latest-json.mjs --tag vX.Y.Z [--bundle-dir DIR] [--url-base URL] [--out FILE] [--notes URL]
//
// Defaults: --bundle-dir target/release/bundle/nsis
//           --url-base   https://github.com/nicolasestrem/screensearch-v2c/releases/download/<tag>/
//           --out        <bundle-dir>/latest.json
//           --notes      https://github.com/nicolasestrem/screensearch-v2c/releases/tag/<tag>
//
// Hard-fails (never emits a manifest) when:
//   • --tag is missing or does not match `v<version>` from tauri.conf.json (catches the
//     four-file hand-synced version drifting from the release tag),
//   • the bundle dir does not contain exactly one `*-setup.exe`,
//   • the matching `<setup>.sig` is absent (i.e. the build was not signed — no
//     TAURI_SIGNING_PRIVATE_KEY), so an unsigned build can never yield a manifest.

import {
  existsSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, "..");
const REPO = "nicolasestrem/screensearch-v2c";

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) {
        throw new Error(`missing value for --${key}`);
      }
      args[key] = next;
      i += 1;
    }
  }
  return args;
}

function fail(message) {
  console.error(`[make-latest-json] ERROR: ${message}`);
  process.exit(1);
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  const tag = args.tag;
  if (!tag) {
    fail("--tag vX.Y.Z is required");
  }
  const tagVersion = tag.startsWith("v") ? tag.slice(1) : tag;

  // Confirm the tag matches the version tauri stamps into the installer name.
  const conf = JSON.parse(
    readFileSync(join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  const confVersion = conf.version;
  if (confVersion !== tagVersion) {
    fail(
      `tag ${tag} (version ${tagVersion}) != tauri.conf.json version ${confVersion}. ` +
        "Bump every version file before tagging (tauri.conf.json, root Cargo.toml, root + ui package.json).",
    );
  }

  const bundleDir =
    args["bundle-dir"] ?? join(repoRoot, "target", "release", "bundle", "nsis");
  if (!existsSync(bundleDir)) {
    fail(`bundle dir not found: ${bundleDir} (did \`tauri build\` run?)`);
  }

  // Exactly one NSIS installer, or we cannot know which to publish.
  const setups = readdirSync(bundleDir).filter((f) =>
    f.toLowerCase().endsWith("-setup.exe"),
  );
  if (setups.length === 0) {
    fail(`no *-setup.exe found in ${bundleDir}`);
  }
  if (setups.length > 1) {
    fail(`expected exactly one *-setup.exe in ${bundleDir}, found: ${setups.join(", ")}`);
  }
  const setup = setups[0];

  // The updater signature Tauri emits beside the installer when createUpdaterArtifacts is on.
  // Its absence means the build was NOT signed — refuse to emit a manifest for it.
  const sigPath = join(bundleDir, `${setup}.sig`);
  if (!existsSync(sigPath)) {
    fail(
      `no signature at ${sigPath} — the build was not signed. ` +
        "Set TAURI_SIGNING_PRIVATE_KEY (+ _PASSWORD) and rebuild with createUpdaterArtifacts enabled.",
    );
  }
  const signature = readFileSync(sigPath, "utf8").trim();
  if (!signature) {
    fail(`signature file is empty: ${sigPath}`);
  }

  const urlBase =
    args["url-base"] ??
    `https://github.com/${REPO}/releases/download/${tag}/`;
  const url = `${urlBase}${encodeURIComponent(setup)}`;
  const notes =
    args.notes ?? `https://github.com/${REPO}/releases/tag/${tag}`;

  // pub_date: RFC 3339. Passed in via --pub-date for a reproducible/CI-stamped value, else now.
  const pubDate = args["pub-date"] ?? new Date().toISOString();

  const manifest = {
    version: tagVersion,
    notes,
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": { signature, url },
    },
  };

  const out = args.out ?? join(bundleDir, "latest.json");
  writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`[make-latest-json] wrote ${out}`);
  console.log(`  version:   ${tagVersion}`);
  console.log(`  installer: ${setup}`);
  console.log(`  url:       ${url}`);
}

main();
