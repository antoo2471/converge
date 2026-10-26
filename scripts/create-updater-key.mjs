import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
const configFile = 'src-tauri/tauri.conf.json';
const config = JSON.parse(fs.readFileSync(configFile, 'utf8'));
const directory = path.join(os.homedir(), '.converge-release');
const key = path.join(directory, 'updater.key');
if (!fs.existsSync(key)) {
  if (config.plugins?.updater?.pubkey) throw new Error('Une clé publique est déjà configurée. Restaurez sa clé privée ; ne faites pas de rotation accidentelle.');
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  if (process.platform === 'win32') {
    const user = `${process.env.USERDOMAIN}\\${process.env.USERNAME}`;
    const acl = spawnSync('icacls.exe', [directory, '/inheritance:r', '/grant:r', `${user}:(OI)(CI)F`], { stdio: 'ignore', windowsHide: true });
    if (acl.status !== 0) throw new Error('Impossible de protéger le dossier de signature.');
  }
  const result = spawnSync(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', 'signer', 'generate', '--ci', '-p', '', '-w', key], { stdio: 'ignore', windowsHide: true });
  if (result.status !== 0) throw new Error('La génération de la clé Tauri a échoué. Aucun secret n’est affiché.');
  fs.chmodSync(key, 0o600);
}
const publicKey = fs.readFileSync(`${key}.pub`, 'utf8').trim();
config.plugins ||= {}; config.plugins.updater ||= {};
if (config.plugins.updater.pubkey && config.plugins.updater.pubkey !== publicKey) throw new Error('La clé privée ne correspond pas à la clé publique du produit.');
config.plugins.updater.pubkey = publicKey;
fs.writeFileSync(configFile, JSON.stringify(config, null, 2) + '\n');
console.log(`Clé publique configurée. Clé privée protégée dans ${key}. Ne la commitez jamais. Configurez-la comme secret GitHub TAURI_SIGNING_PRIVATE_KEY.`);
