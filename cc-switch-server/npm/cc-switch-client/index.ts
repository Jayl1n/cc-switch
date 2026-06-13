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
 * ## Management API
 *
 * Besides the proxy, the server exposes a localhost-only management API on
 * an independent port (`info.mgmtPort`). The typed methods below let you manage
 * Skills and MCP servers directly, mirroring `cc-switch-core` services:
 *
 * ```ts
 * // Skills — install / list / toggle / update
 * const skills = await client.listSkills();                    // InstalledSkill[]
 * const installed = await client.installSkill(skill, 'claude');
 * await client.toggleSkillApp(installed.id, 'codex', true);
 * await client.updateSkill(installed.id);
 *
 * // Skill discovery & storage
 * const repos = await client.listSkillRepos();                 // SkillRepo[]
 * const hits = await client.searchSkills('pdf');               // SkillsShSearchResult
 * await client.migrateSkillStorage('unified');                 // MigrationResult
 *
 * // MCP servers — CRUD + sync + import
 * const servers = await client.listMcpServers();               // McpServer[]
 * await client.upsertMcpServer(server);
 * await client.syncMcp();                                      // sync enabled → all apps
 * const n = await client.importMcpFromApp('codex');            // import existing config
 * ```
 *
 * The correct binary for your platform is installed automatically.
 */

import { spawn, type Subprocess } from 'bun';
import { resolveBinaryPath } from './detect-platform';

/** encodeURIComponent 别名，用于管理 API 路径参数编码。 */
const enc = encodeURIComponent;

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

  // ── Management API（/api/v1/，端口来自 ProxyInfo.mgmtPort）──

  /** 列出所有已安装 Skill。 */
  listSkills(): Promise<InstalledSkill[]> {
    return this.mgmtFetch<InstalledSkill[]>('/skills');
  }

  /** 扫描各 app 目录下、未由 cc-switch 管理的 Skill。 */
  listUnmanagedSkills(): Promise<UnmanagedSkill[]> {
    return this.mgmtFetch<UnmanagedSkill[]>('/skills/unmanaged');
  }

  /** 列出所有 Skill 备份。 */
  listSkillBackups(): Promise<SkillBackupEntry[]> {
    return this.mgmtFetch<SkillBackupEntry[]>('/skills/backups');
  }

  /** 列出已注册的 Skill 仓库。 */
  listSkillRepos(): Promise<SkillRepo[]> {
    return this.mgmtFetch<SkillRepo[]>('/skill-repos');
  }

  /** 从仓库安装一个 Skill 到指定 app。 */
  installSkill(skill: DiscoverableSkill, app: AppType): Promise<InstalledSkill> {
    return this.mgmtPost<InstalledSkill>('/skills/install', { skill, app });
  }

  /** 从本地 zip 文件安装 Skill（path 为同机绝对路径）。 */
  installSkillFromZip(path: string, app: AppType): Promise<InstalledSkill[]> {
    return this.mgmtPost<InstalledSkill[]>('/skills/install-zip', { path, app });
  }

  /** 卸载 Skill（返回备份路径，若有）。 */
  uninstallSkill(id: string): Promise<SkillUninstallResult> {
    return this.mgmtDelete<SkillUninstallResult>(`/skills/${enc(id)}`);
  }

  /** 启用/禁用 Skill 在某 app 的同步。 */
  toggleSkillApp(id: string, app: AppType, enabled: boolean): Promise<void> {
    return this.mgmtPost<void>(`/skills/${enc(id)}/toggle`, { app, enabled });
  }

  /** 检查所有已安装 Skill 的远程更新。 */
  checkSkillUpdates(): Promise<SkillUpdateInfo[]> {
    return this.mgmtPost<SkillUpdateInfo[]>('/skills/check-updates');
  }

  /** 更新单个 Skill 到最新版。 */
  updateSkill(id: string): Promise<InstalledSkill> {
    return this.mgmtPost<InstalledSkill>(`/skills/${enc(id)}/update`);
  }

  /** 批量导入已有 Skill（按 directory + apps 选择）。 */
  importSkills(imports: ImportSkillSelection[]): Promise<InstalledSkill[]> {
    return this.mgmtPost<InstalledSkill[]>('/skills/import', { imports });
  }

  /** 迁移 Skill 存储位置。 */
  migrateSkillStorage(target: SkillStorageLocation): Promise<MigrationResult> {
    return this.mgmtPost<MigrationResult>('/skills/migrate-storage', { target });
  }

  /** 在 skills.sh 搜索 Skill。 */
  searchSkills(query: string, limit = 20, offset = 0): Promise<SkillsShSearchResult> {
    return this.mgmtPost<SkillsShSearchResult>('/skills/search', { query, limit, offset });
  }

  /** 删除一个 Skill 备份。 */
  deleteSkillBackup(backupId: string): Promise<void> {
    return this.mgmtDelete<void>(`/skills/backups/${enc(backupId)}`);
  }

  /** 从备份恢复 Skill 到指定 app。 */
  restoreSkillBackup(backupId: string, app: AppType): Promise<InstalledSkill> {
    return this.mgmtPost<InstalledSkill>(`/skills/backups/${enc(backupId)}/restore`, { app });
  }

  /** 添加一个 Skill 仓库。 */
  addSkillRepo(repo: SkillRepo): Promise<void> {
    return this.mgmtPost<void>('/skill-repos', repo);
  }

  /** 移除一个 Skill 仓库。 */
  removeSkillRepo(owner: string, name: string): Promise<void> {
    return this.mgmtDelete<void>(`/skill-repos/${enc(owner)}/${enc(name)}`);
  }

  // ── MCP ──

  /** 列出所有 MCP server 配置。 */
  listMcpServers(): Promise<McpServer[]> {
    return this.mgmtFetch<McpServer[]>('/mcp/servers');
  }

  /** 新增/更新一个 MCP server 配置。 */
  upsertMcpServer(server: McpServer): Promise<void> {
    return this.mgmtPost<void>('/mcp/servers', server);
  }

  /** 删除一个 MCP server，返回是否已移除。 */
  deleteMcpServer(id: string): Promise<boolean> {
    return this.mgmtDelete<boolean>(`/mcp/servers/${enc(id)}`);
  }

  /** 启用/禁用 MCP server 在某 app 的同步。 */
  toggleMcpApp(id: string, app: AppType, enabled: boolean): Promise<void> {
    return this.mgmtPost<void>(`/mcp/servers/${enc(id)}/toggle`, { app, enabled });
  }

  /** 将所有启用的 MCP 配置同步到各 app。 */
  syncMcp(): Promise<void> {
    return this.mgmtPost<void>('/mcp/sync');
  }

  /** 从某 app 的现有配置导入 MCP server（返回导入条数）。 */
  importMcpFromApp(app: AppType): Promise<number> {
    return this.mgmtPost<number>(`/mcp/import/${enc(app)}`);
  }

  // ── Management API 内部辅助 ──

  private mgmtUrl(path: string): string {
    if (!this.proxyInfo) {
      throw new Error('Proxy not started. Call start() first.');
    }
    return `http://${this.proxyInfo.address}:${this.proxyInfo.mgmtPort}/api/v1${path}`;
  }

  private async mgmtFetch<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(this.mgmtUrl(path), init);
    if (!res.ok) {
      const body = await res.text().catch(() => '');
      throw new Error(`mgmt API ${path} failed: HTTP ${res.status} ${body}`);
    }
    const text = await res.text();
    return (text ? JSON.parse(text) : null) as T;
  }

  private mgmtPost<T>(path: string, body?: unknown): Promise<T> {
    return this.mgmtFetch<T>(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  }

  private mgmtDelete<T>(path: string): Promise<T> {
    return this.mgmtFetch<T>(path, { method: 'DELETE' });
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
