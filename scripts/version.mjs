// The app's version lives in package.json; tauri.conf.json reads it from there. This keeps
// the other places that carry it (Cargo.toml, Cargo.lock, package-lock.json) in step.
//
//   node scripts/version.mjs              print the version
//   node scripts/version.mjs 0.2.0        set it
//   node scripts/version.mjs minor        new features: 0.2.3 -> 0.3.0
//   node scripts/version.mjs patch        fixes only:   0.2.3 -> 0.2.4
//   node scripts/version.mjs --check [v0.2.0]
//        fail unless every file agrees, the release notes exist, and (when given) the tag is
//        v<version>; the release workflow runs this.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const file = (p) => path.join(root, p);
const read = (p) => fs.readFileSync(file(p), 'utf8');
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

const CARGO_TOML = /^(\[package\][^[]*?\nversion = ")([^"]+)(")/m;
const CARGO_LOCK = /(\[\[package\]\]\nname = "ds2-save-manager"\nversion = ")([^"]+)(")/;

function current() {
  return JSON.parse(read('package.json')).version;
}

/** Every place the version is written, as [file, version found there]. */
function found() {
  const lock = JSON.parse(read('package-lock.json'));
  const eol = (s) => s.replace(/\r\n/g, '\n');
  return [
    ['package.json', current()],
    ['package-lock.json', lock.version],
    ['package-lock.json (packages[""])', lock.packages?.['']?.version],
    ['src-tauri/Cargo.toml', CARGO_TOML.exec(eol(read('src-tauri/Cargo.toml')))?.[2]],
    ['src-tauri/Cargo.lock', CARGO_LOCK.exec(eol(read('src-tauri/Cargo.lock')))?.[2]]
  ];
}

function setVersion(version) {
  const keepEol = (p, change) => {
    const text = read(p);
    const crlf = text.includes('\r\n');
    let out = change(text.replace(/\r\n/g, '\n'));
    if (crlf) out = out.replace(/\n/g, '\r\n');
    fs.writeFileSync(file(p), out);
  };
  const json = (p, change) =>
    keepEol(p, (text) => {
      const data = JSON.parse(text);
      change(data);
      return JSON.stringify(data, null, 2) + '\n';
    });
  json('package.json', (d) => (d.version = version));
  json('package-lock.json', (d) => {
    d.version = version;
    d.packages[''].version = version;
  });
  const replace = (re) => (text) => {
    if (!re.test(text)) throw new Error(`no version found by ${re}`);
    return text.replace(re, `$1${version}$3`);
  };
  keepEol('src-tauri/Cargo.toml', replace(CARGO_TOML));
  keepEol('src-tauri/Cargo.lock', replace(CARGO_LOCK));
}

function check(tag) {
  const version = current();
  const problems = [];
  if (!SEMVER.test(version)) problems.push(`package.json: "${version}" is not MAJOR.MINOR.PATCH`);
  for (const [where, v] of found()) if (v !== version) problems.push(`${where}: ${v ?? 'no version'} (package.json says ${version})`);
  for (const lang of ['en', 'tr']) {
    if (!fs.existsSync(file(`changelog/${version}.${lang}.md`))) problems.push(`changelog/${version}.${lang}.md is missing`);
  }
  if (tag && tag !== `v${version}`) problems.push(`tag ${tag} is not v${version}`);
  if (problems.length) {
    console.error(problems.map((p) => `- ${p}`).join('\n'));
    process.exit(1);
  }
  console.log(`version ${version}: every file agrees${tag ? `, tag ${tag} matches` : ''}`);
}

const [arg, extra] = process.argv.slice(2);
if (!arg) {
  console.log(current());
} else if (arg === '--check') {
  check(extra);
} else {
  const [major, minor, patch] = current().split('.').map(Number);
  const next = arg === 'minor' ? `${major}.${minor + 1}.0` : arg === 'patch' ? `${major}.${minor}.${patch + 1}` : arg.replace(/^v/, '');
  if (!SEMVER.test(next)) {
    console.error(`"${arg}" is not a version (MAJOR.MINOR.PATCH), "minor" or "patch"`);
    process.exit(1);
  }
  setVersion(next);
  console.log(`${current()} set in ${found().map(([f]) => f).join(', ')}; tauri.conf.json reads package.json`);
}
