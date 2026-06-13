/**
 * CC Switch Client — Bun/Node.js client for cc-switch-server subprocess
 *
 * ## Quick Start
 *
 * ```ts
 * import { CCSwitchClient } from 'cc-switch-client';
 *
 * const client = new CCSwitchClient({ configDir: '/my/app/data' });
 * const info = await client.start();
 * console.log(`Proxy running on ${info.address}:${info.port}`);
 *
 * // All AI tool requests go through http://127.0.0.1:{info.port}
 * // The proxy handles routing, failover, model mapping, etc.
 *
 * await client.stop();
 * ```
 *
 * ## Install
 *
 * ```bash
 * bun add cc-switch-client
 * # or
 * npm install cc-switch-client
 * ```
 *
 * The correct binary for your platform is installed automatically.
 */

import { spawn, type Subprocess } from 'bun';
import { resolveBinaryPath } from './detect-platform';

export interface CCSwitchConfig {
  /** Path to cc-switch-server binary (default: auto-detect from platform package) */
  binaryPath?: string;

  /** Configuration directory (default: ~/.cc-switch/) */
  configDir?: string;

  /** Proxy listen port (0 = OS assigns free port, default: 0) */
  port?: number;

  /** Listen address (default: '127.0.0.1') */
  listenAddr?: string;

  /** Enable Live config takeover (rewrites tool configs to point at proxy) */
  takeover?: boolean;

  /** Log level: trace | debug | info | warn | error (default: 'warn') */
  logLevel?: string;

  /** Timeout in ms waiting for the server to start (default: 10000) */
  startTimeout?: number;
}

export interface ProxyInfo {
  status: string;
  address: string;
  port: number;
  started_at: string;
  takeover: boolean;
  /** 管理 API 端口（/api/v1/），由 server 启动 JSON 提供 */
  mgmtPort: number;
}

// ── Skill / MCP 类型（镜像 cc-switch-core 的 serde camelCase 输出）──

export interface SkillApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
  opencode: boolean;
  hermes: boolean;
}

export interface InstalledSkill {
  id: string;
  name: string;
  description?: string;
  directory: string;
  repoOwner?: string;
  repoName?: string;
  repoBranch?: string;
  readmeUrl?: string;
  apps: SkillApps;
  installedAt: number;
  contentHash?: string;
  updatedAt: number;
}

export interface UnmanagedSkill {
  directory: string;
  name: string;
  description?: string;
  foundIn: string[];
  path: string;
}

export interface SkillBackupEntry {
  backupId: string;
  backupPath: string;
  createdAt: number;
  skill: InstalledSkill;
}

export interface SkillRepo {
  owner: string;
  name: string;
  branch: string;
  enabled: boolean;
}

export interface DiscoverableSkill {
  key: string;
  name: string;
  description: string;
  directory: string;
  readmeUrl?: string;
  repoOwner: string;
  repoName: string;
  repoBranch: string;
}

export interface SkillUpdateInfo {
  id: string;
  name: string;
  currentHash?: string;
  remoteHash: string;
}

export interface SkillUninstallResult {
  backupPath?: string;
}

export interface ImportSkillSelection {
  directory: string;
  apps: SkillApps;
}

export interface SkillsShDiscoverableSkill {
  key: string;
  name: string;
  directory: string;
  repoOwner: string;
  repoName: string;
  repoBranch: string;
  installs: number;
  readmeUrl?: string;
}

export interface SkillsShSearchResult {
  skills: SkillsShDiscoverableSkill[];
  totalCount: number;
  query: string;
}

export interface MigrationResult {
  migratedCount: number;
  skippedCount: number;
  errors: string[];
}

export type AppType = 'claude' | 'codex' | 'gemini' | 'opencode' | 'hermes';
export type SkillStorageLocation = 'cc_switch' | 'unified';

export interface McpApps {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
  opencode: boolean;
  hermes: boolean;
}

export interface McpServer {
  id: string;
  name: string;
  server: Record<string, unknown>;
  apps: McpApps;
  description?: string;
  homepage?: string;
  docs?: string;
  tags?: string[];
}

export class CCSwitchClient {
  private config: Required<Pick<CCSwitchConfig, 'port' | 'listenAddr' | 'logLevel' | 'startTimeout' | 'takeover'>> & CCSwitchConfig;
  private proc: Subprocess | null = null;
  private proxyInfo: ProxyInfo | null = null;

  constructor(config: CCSwitchConfig = {}) {
    this.config = {
      binaryPath: config.binaryPath,
      configDir: config.configDir,
      port: config.port ?? 0,
      listenAddr: config.listenAddr ?? '127.0.0.1',
      takeover: config.takeover ?? false,
      logLevel: config.logLevel ?? 'warn',
      startTimeout: config.startTimeout ?? 10000,
    };
  }

  /**
   * Start the cc-switch-server subprocess.
   * Returns proxy connection info (address + port).
   */
  async start(): Promise<ProxyInfo> {
    if (this.proc) {
      throw new Error('cc-switch-server is already running');
    }

    const binary = this.config.binaryPath || resolveBinaryPath();
    const args = this.buildArgs();

    this.proc = spawn({
      cmd: [binary, ...args],
      stdout: 'pipe',
      stderr: 'pipe',
      stdin: 'ignore',
    });

    this.proxyInfo = await this.readStartupMessage();
    return this.proxyInfo;
  }

  /**
   * Get the proxy URL for use in tool configuration.
   * e.g., "http://127.0.0.1:54321"
   */
  getProxyUrl(): string {
    if (!this.proxyInfo) {
      throw new Error('Proxy not started. Call start() first.');
    }
    return `http://${this.proxyInfo.address}:${this.proxyInfo.port}`;
  }

  /**
   * Get the proxy port (0 if not started).
   */
  getPort(): number {
    return this.proxyInfo?.port ?? 0;
  }

  /**
   * Get the full proxy info.
   */
  getInfo(): ProxyInfo | null {
    return this.proxyInfo;
  }

  /**
   * Check if the server process is running.
   */
  isRunning(): boolean {
    return this.proc !== null && this.proc.exitCode === null;
  }

  /**
   * Stop the server gracefully.
   */
  async stop(): Promise<void> {
    if (!this.proc) return;

    this.proc.kill('SIGTERM');

    const timeout = setTimeout(() => {
      if (this.proc && this.proc.exitCode === null) {
        this.proc.kill('SIGKILL');
      }
    }, 5000);

    try {
      await this.proc.exited;
    } finally {
      clearTimeout(timeout);
      this.proc = null;
      this.proxyInfo = null;
    }
  }

  // ── Private helpers ──

  private buildArgs(): string[] {
    const args: string[] = [];

    if (this.config.configDir) {
      args.push('--config-dir', this.config.configDir);
    }
    args.push('--port', String(this.config.port));
    args.push('--listen-addr', this.config.listenAddr!);
    args.push('--log-level', this.config.logLevel!);
    args.push('--output', 'json');

    if (this.config.takeover) {
      args.push('--takeover');
    }

    return args;
  }

  private async readStartupMessage(): Promise<ProxyInfo> {
    const stdout = this.proc!.stdout;
    // bun 的 Subprocess.stdout 类型为 number | ReadableStream | undefined
    // （number = 原始 fd）；spawn(stdout: 'pipe') 运行时恒为 ReadableStream
    if (stdout == null || typeof stdout === 'number') {
      throw new Error('Failed to open stdout pipe');
    }

    const reader = stdout.getReader();
    const decoder = new TextDecoder();

    const deadline = Date.now() + this.config.startTimeout!;
    let buffer = '';

    while (Date.now() < deadline) {
      const { value, done } = await reader.read();
      if (done) {
        throw new Error('Server process exited before sending status');
      }

      buffer += decoder.decode(value, { stream: true });

      for (const line of buffer.split('\n')) {
        const trimmed = line.trim();
        if (trimmed.startsWith('{')) {
          try {
            const info = JSON.parse(trimmed);
            if (info.status === 'running') {
              reader.releaseLock();
              return info as ProxyInfo;
            }
          } catch {
            // Not valid JSON yet
          }
        }
      }
    }

    reader.releaseLock();
    throw new Error(`Server did not start within ${this.config.startTimeout}ms`);
  }
}

/**
 * Convenience: create and start a proxy in one call.
 */
export async function startProxy(config?: CCSwitchConfig): Promise<{ client: CCSwitchClient; info: ProxyInfo }> {
  const client = new CCSwitchClient(config);
  const info = await client.start();
  return { client, info };
}
