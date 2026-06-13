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

describe('CCSwitchClient management API', () => {
  test('ProxyInfo exposes independent mgmtPort', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });
    const info = await client.start();
    expect(info.mgmtPort).toBeGreaterThan(0);
    // 管理端口与代理端口是两个独立的 OS 分配端口
    expect(info.mgmtPort).not.toBe(info.port);
    await client.stop();
  });

  test('list endpoints return arrays on empty config', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });
    await client.start();

    expect(await client.listSkills()).toEqual([]);
    expect(Array.isArray(await client.listUnmanagedSkills())).toBe(true);
    expect(await client.listSkillBackups()).toEqual([]);
    expect(Array.isArray(await client.listSkillRepos())).toBe(true);
    expect(Array.isArray(await client.listMcpServers())).toBe(true);

    await client.stop();
  });

  test('mcp sync/import are safe on empty config', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });
    await client.start();

    await client.syncMcp(); // 空配置，不抛错
    const imported = await client.importMcpFromApp('claude');
    expect(typeof imported).toBe('number');

    await client.stop();
  });

  test('migrateSkillStorage to current target is no-op', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });
    await client.start();

    // 默认存储位置即 cc_switch，迁移到同位置 → 0 迁移
    const result = await client.migrateSkillStorage('cc_switch');
    expect(result.migratedCount).toBe(0);

    await client.stop();
  });

  test('checkSkillUpdates returns array', async () => {
    const client = new CCSwitchClient({
      configDir: makeConfigDir(),
      port: 0,
      logLevel: 'warn',
    });
    await client.start();

    const updates = await client.checkSkillUpdates();
    expect(Array.isArray(updates)).toBe(true);

    await client.stop();
  });

  test('management methods reject before start', async () => {
    const client = new CCSwitchClient({ configDir: makeConfigDir() });
    // mgmtUrl 在 proxyInfo 为空时抛错；async 方法将其转为 rejected promise
    await expect(client.listSkills()).rejects.toThrow('not started');
  });
});
