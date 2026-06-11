import { test, expect, describe, afterAll } from 'bun:test';
import { CCSwitchClient, startProxy } from '../index';
import { resolveBinaryPath, detectPlatformPackage } from '../detect-platform';
import { join } from 'path';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';

const tmpDirs: string[] = [];

function makeConfigDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'cc-switch-test-'));
  tmpDirs.push(dir);
  return dir;
}

afterAll(() => {
  for (const dir of tmpDirs) {
    try { rmSync(dir, { recursive: true }); } catch {}
  }
});

describe('detect-platform', () => {
  test('detects current platform package', () => {
    const pkg = detectPlatformPackage();
    // This test runs on macOS ARM64 in CI
    expect(pkg).toBeTruthy();
    if (process.platform === 'darwin' && process.arch === 'arm64') {
      expect(pkg).toBe('cc-switch-client-darwin-arm64');
    }
  });

  test('resolveBinaryPath returns a valid path', () => {
    const path = resolveBinaryPath();
    expect(path).toContain('cc-switch-client-');
    expect(path).toContain('bin');
  });
});

describe('CCSwitchClient', () => {
  test('starts and stops proxy', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });

    const info = await client.start();
    expect(info.status).toBe('running');
    expect(info.port).toBeGreaterThan(0);
    expect(info.address).toBeTruthy();
    expect(client.isRunning()).toBe(true);

    await client.stop();
    expect(client.isRunning()).toBe(false);
  });

  test('getProxyUrl returns correct URL', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });

    await client.start();
    const url = client.getProxyUrl();
    expect(url).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);

    await client.stop();
  });

  test('proxy responds to HTTP requests', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });

    await client.start();
    const url = client.getProxyUrl();

    const res = await fetch(url);
    expect(res.status).toBeDefined();

    await client.stop();
  });

  test('startProxy convenience function works', async () => {
    const { client, info } = await startProxy({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });

    expect(info.status).toBe('running');
    expect(info.port).toBeGreaterThan(0);

    await client.stop();
  });

  test('stop is idempotent', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });

    await client.start();
    await client.stop();
    await client.stop(); // should not throw
  });

  test('getProxyUrl throws before start', () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
    });
    expect(() => client.getProxyUrl()).toThrow('not started');
  });
});
