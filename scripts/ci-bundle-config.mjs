import fs from 'node:fs';
const release = process.env.SIGN_UPDATES === 'true';
if (release && !process.env.TAURI_SIGNING_PRIVATE_KEY) throw new Error('Configurez le secret TAURI_SIGNING_PRIVATE_KEY avant de lancer une release.');
fs.writeFileSync('src-tauri/tauri.ci.conf.json', JSON.stringify({ bundle: { createUpdaterArtifacts: release } }));
