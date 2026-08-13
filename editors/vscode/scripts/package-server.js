'use strict';

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const extensionRoot = path.resolve(__dirname, '..');
const repositoryRoot = path.resolve(extensionRoot, '..', '..');
const executable = process.platform === 'win32'
  ? 'terrane-language-server.exe'
  : 'terrane-language-server';

const build = spawnSync(
  'cargo',
  ['build', '--release', '--package', 'terrane-language-server'],
  { cwd: repositoryRoot, stdio: 'inherit' },
);
if (build.error) {
  throw build.error;
}
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

const targetRoot = process.env.CARGO_TARGET_DIR
  ? path.resolve(repositoryRoot, process.env.CARGO_TARGET_DIR)
  : path.join(repositoryRoot, 'target');
const source = path.join(targetRoot, 'release', executable);
const destinationDirectory = path.join(extensionRoot, 'server');
const destination = path.join(destinationDirectory, executable);
fs.mkdirSync(destinationDirectory, { recursive: true });
fs.copyFileSync(source, destination);
if (process.platform !== 'win32') {
  fs.chmodSync(destination, 0o755);
}
console.log(`Packaged ${destination}`);
