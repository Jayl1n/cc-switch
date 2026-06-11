#!/usr/bin/env node
/**
 * prepublish.js — Called by `npm publish` / `bun publish` automatically.
 *
 * Replaces `file:../xxx` optionalDependencies with version numbers
 * so the published package points to npm registry, not local paths.
 */

const fs = require('fs');
const path = require('path');

const pkgPath = path.join(__dirname, '..', 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));

const version = pkg.version;
let changed = false;

for (const [dep, value] of Object.entries(pkg.optionalDependencies || {})) {
  if (typeof value === 'string' && value.startsWith('file:')) {
    pkg.optionalDependencies[dep] = version;
    changed = true;
  }
}

if (changed) {
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`prepublish: resolved optionalDependencies to version ${version}`);
} else {
  console.log('prepublish: optionalDependencies already resolved');
}
