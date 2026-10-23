// The notes of a draft release: every changelog newer than the last published release, up to
// the version in package.json, newest first, each under its version, in English and then in
// Turkish. So when versions are skipped between releases (0.2.0 published, then 0.4.0), the
// notes of the ones in between (0.3.0) still reach a release. Prints the notes and changes
// nothing; the release workflow runs it with --github, and the same command run by hand shows
// what the next draft will say.
//
//   node scripts/release-notes.mjs                   nothing published yet: every version's notes
//   node scripts/release-notes.mjs v0.2.0 v0.4.0     as if these releases were published
//   node scripts/release-notes.mjs --github          the repository's published releases (gh CLI;
//                                                    $GITHUB_REPOSITORY names the repository if set)
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const NOTES = /^((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))\.(en|tr)\.md$/;

/** Negative when version a is older than b. */
function compare(a, b) {
  const [x, y] = [a, b].map((v) => v.split('.').map(Number));
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
}

/** Tags of the releases that are out (drafts are not), as GitHub lists them. */
function publishedOnGitHub() {
  const repo = process.env.GITHUB_REPOSITORY ? ['--repo', process.env.GITHUB_REPOSITORY] : [];
  const out = execFileSync('gh', ['release', 'list', ...repo, '--exclude-drafts', '--limit', '1000', '--json', 'tagName', '--jq', '.[].tagName'], {
    cwd: root,
    encoding: 'utf8'
  });
  return out.split('\n').map((t) => t.trim()).filter(Boolean);
}

const current = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const args = process.argv.slice(2);
const tags = args[0] === '--github' ? publishedOnGitHub() : args;

// The newest published release older than this one; a later one (an older branch run by hand)
// says nothing about what this release adds.
const last = tags
  .map((t) => t.replace(/^v/, ''))
  .filter((v) => SEMVER.test(v) && compare(v, current) < 0)
  .sort(compare)
  .at(-1);

const written = new Set(fs.readdirSync(path.join(root, 'changelog')).map((f) => NOTES.exec(f)?.[1]).filter(Boolean));
const versions = [...written].filter((v) => compare(v, current) <= 0 && (!last || compare(v, last) > 0)).sort((a, b) => compare(b, a));
if (versions[0] !== current) {
  console.error(`changelog/${current}.en.md and .tr.md are missing`);
  process.exit(1);
}

const notes = (version, lang) => {
  const file = path.join(root, 'changelog', `${version}.${lang}.md`);
  if (!fs.existsSync(file)) {
    console.error(`changelog/${version}.${lang}.md is missing`);
    process.exit(1);
  }
  return fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n').trim();
};
const section = (lang) => versions.map((v) => `## ${v}\n\n${notes(v, lang)}`).join('\n\n');
const body = `# English\n\n${section('en')}\n\n# Türkçe\n\n${section('tr')}\n`;

console.error(`notes of ${versions.join(', ')} (${last ? `last published: v${last}` : 'no earlier release published'})`);
process.stdout.write(body);
