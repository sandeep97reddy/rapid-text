import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

function bumpPatchVersion(version) {
  const parts = version.split('.');
  if (parts.len < 3) return version;
  const patch = parseInt(parts[2], 10);
  if (isNaN(patch)) return version;
  parts[2] = (patch + 1).toString();
  return parts.join('.');
}

try {
  // 1. package.json
  const pkgPath = path.join(rootDir, 'package.json');
  const pkgData = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
  const oldVersion = pkgData.version;
  const newVersion = bumpPatchVersion(oldVersion);
  pkgData.version = newVersion;
  fs.writeFileSync(pkgPath, JSON.stringify(pkgData, null, 2) + '\n');

  // 2. tauri.conf.json
  const tauriPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
  if (fs.existsSync(tauriPath)) {
    const tauriData = JSON.parse(fs.readFileSync(tauriPath, 'utf8'));
    tauriData.version = newVersion;
    fs.writeFileSync(tauriPath, JSON.stringify(tauriData, null, 2) + '\n');
  }

  // 3. Cargo.toml
  const cargoPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
  if (fs.existsSync(cargoPath)) {
    let cargoContent = fs.readFileSync(cargoPath, 'utf8');
    cargoContent = cargoContent.replace(/version\s*=\s*"[^"]+"/, `version = "${newVersion}"`);
    fs.writeFileSync(cargoPath, cargoContent);
  }

  console.log(`[Rapid Text] Auto-bumped version: ${oldVersion} -> ${newVersion}`);
} catch (err) {
  console.error('[Rapid Text] Version bump error:', err);
}
