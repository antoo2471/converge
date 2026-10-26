import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export function nextVersion(date, tags) {
  const parts = new Intl.DateTimeFormat('en', { timeZone: 'Europe/Paris', year: 'numeric', month: 'numeric' }).formatToParts(date);
  const year = Number(parts.find(p => p.type === 'year').value) % 100;
  const month = Number(parts.find(p => p.type === 'month').value);
  const releases = tags.map(tag => /^v?(\d+)\.(\d+)\.(\d+)$/.exec(tag.trim())).filter(Boolean)
    .filter(match => Number(match[1]) === year && Number(match[2]) === month).map(match => Number(match[3]));
  return `${year}.${month}.${Math.max(0, ...releases) + 1}`;
}
export function writeVersion(version, directory = root) {
  if (!/^\d{2}\.(?:[1-9]|1[0-2])\.[1-9]\d*$/.test(version)) throw new Error('Version attendue : année.mois.release, par exemple 26.10.1');
  for (const file of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']) {
    const filename = path.join(directory, file); const value = JSON.parse(fs.readFileSync(filename, 'utf8'));
    value.version = version;
    if (value.packages?.['']) value.packages[''].version = version;
    fs.writeFileSync(filename, JSON.stringify(value, null, 2) + '\n');
  }
  for (const [file, pattern] of [['src-tauri/Cargo.toml', /(\[package\][\s\S]*?\nversion\s*=\s*)"[^"]+"/], ['src-tauri/Cargo.lock', /(\[\[package\]\]\r?\nname = "converge"\r?\nversion = )"[^"]+"/]]) {
    const filename = path.join(directory, file); const content = fs.readFileSync(filename, 'utf8');
    if (!pattern.test(content)) throw new Error(`Version Converge introuvable dans ${file}`);
    fs.writeFileSync(filename, content.replace(pattern, `$1"${version}"`));
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const explicit = process.argv.find(arg => arg.startsWith('--version='))?.slice(10);
  const tags = execFileSync('git', ['tag', '--list'], { cwd: root, encoding: 'utf8' }).split('\n');
  const version = explicit || nextVersion(new Date(), tags);
  if (process.argv.includes('--write')) writeVersion(version);
  if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\ntag=v${version}\n`);
  console.log(version);
}
