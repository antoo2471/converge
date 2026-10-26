import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nextVersion, writeVersion } from './release-version.mjs';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
test('initial release and monthly numbering', () => {
  assert.equal(nextVersion(new Date('2026-10-26T12:00:00Z'), []), '26.10.1');
  assert.equal(nextVersion(new Date('2026-10-26T12:00:00Z'), ['v26.10.1','v26.10.4','v26.9.99','v26.10.2-beta']), '26.10.5');
  assert.equal(nextVersion(new Date('2026-11-02T12:00:00Z'), ['v26.10.9']), '26.11.1');
  assert.equal(nextVersion(new Date('2026-10-31T23:30:00Z'), ['v26.10.9']), '26.11.1');
});
test('writes all manifests without changing other package versions', () => {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'converge-version-'));
  try {
    fs.mkdirSync(path.join(root,'src-tauri'));
    fs.writeFileSync(path.join(root,'package.json'),JSON.stringify({name:'converge',version:'0.1.0'}));
    fs.writeFileSync(path.join(root,'package-lock.json'),JSON.stringify({version:'0.1.0',packages:{'':{version:'0.1.0'},'node_modules/keep':{version:'9.0.0'}}}));
    fs.writeFileSync(path.join(root,'src-tauri/tauri.conf.json'),JSON.stringify({version:'0.1.0',identifier:'app.converge.desktop'}));
    fs.writeFileSync(path.join(root,'src-tauri/Cargo.toml'),'[package]\nname = "converge"\nversion = "0.1.0"\n[dependencies]\nkeep = "9.0.0"\n');
    fs.writeFileSync(path.join(root,'src-tauri/Cargo.lock'),'[[package]]\nname = "keep"\nversion = "9.0.0"\n\n[[package]]\nname = "converge"\nversion = "0.1.0"\n');
    writeVersion('26.10.1',root);
    for (const file of ['package.json','package-lock.json','src-tauri/tauri.conf.json']) assert.equal(JSON.parse(fs.readFileSync(path.join(root,file))).version,'26.10.1');
    const lock=JSON.parse(fs.readFileSync(path.join(root,'package-lock.json')));
    assert.equal(lock.packages[''].version,'26.10.1');assert.equal(lock.packages['node_modules/keep'].version,'9.0.0');
    assert.match(fs.readFileSync(path.join(root,'src-tauri/Cargo.lock'),'utf8'),/name = "converge"\nversion = "26.10.1"/);
    assert.match(fs.readFileSync(path.join(root,'src-tauri/Cargo.lock'),'utf8'),/name = "keep"\nversion = "9.0.0"/);
    assert.match(fs.readFileSync(path.join(root,'src-tauri/Cargo.toml'),'utf8'),/version = "26.10.1"/);
  } finally {
    assert.equal(fs.realpathSync(path.dirname(root)),fs.realpathSync(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('converge-version-'));
    fs.rmSync(root,{recursive:true,force:true});
  }
});
