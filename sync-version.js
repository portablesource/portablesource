import fs from 'fs';
import path from 'path';

// Read version from version.json
const versionData = JSON.parse(fs.readFileSync('version.json', 'utf8'));
const version = versionData.version;

console.log(`Syncing version ${version} across all config files...`);

// Update package.json
const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf8'));
packageJson.version = version;
fs.writeFileSync('package.json', JSON.stringify(packageJson, null, '\t'));
console.log('✓ Updated package.json');

// Update tauri.conf.json
const tauriConf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
tauriConf.version = version;
fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(tauriConf, null, 2));
console.log('✓ Updated src-tauri/tauri.conf.json');

// Update Cargo.toml
const cargoToml = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
const updatedCargoToml = cargoToml.replace(/^version = ".*"$/m, `version = "${version}"`);
fs.writeFileSync('src-tauri/Cargo.toml', updatedCargoToml);
console.log('✓ Updated src-tauri/Cargo.toml');

// Update package-lock.json
const packageLockJson = JSON.parse(fs.readFileSync('package-lock.json', 'utf8'));
packageLockJson.version = version;
packageLockJson.packages[''].version = version;
fs.writeFileSync('package-lock.json', JSON.stringify(packageLockJson, null, '\t'));
console.log('✓ Updated package-lock.json');

console.log('Version sync completed!');