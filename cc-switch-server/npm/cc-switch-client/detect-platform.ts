/**
 * Detect the current platform's npm package name for the cc-switch-server binary.
 * Returns null if the platform is not supported.
 */
export function detectPlatformPackage(): string | null {
  const platformKey = `${process.platform}-${process.arch}`;

  const platformMap: Record<string, string> = {
    'darwin-arm64': 'cc-switch-client-darwin-arm64',
    'darwin-x64': 'cc-switch-client-darwin-x64',
    'linux-x64': 'cc-switch-client-linux-x64',
    'linux-arm64': 'cc-switch-client-linux-arm64',
    'win32-x64': 'cc-switch-client-win32-x64',
  };

  return platformMap[platformKey] ?? null;
}

/**
 * Resolve the absolute path to the cc-switch-server binary
 * by finding the installed platform-specific package.
 */
export function resolveBinaryPath(): string {
  const pkgName = detectPlatformPackage();
  if (!pkgName) {
    throw new Error(
      `Unsupported platform: ${process.platform}-${process.arch}. ` +
      `Supported: darwin-arm64, darwin-x64, linux-x64, linux-arm64, win32-x64`
    );
  }

  // Require the platform package to resolve its location
  let pkgDir: string;
  try {
    // Resolve the package's entry point directory
    const resolved = require.resolve(pkgName + '/package.json');
    pkgDir = resolved.replace(/\/package\.json$/, '');
  } catch {
    throw new Error(
      `Platform package "${pkgName}" is not installed. ` +
      `Run: bun install (or npm install) to fetch the binary for your platform.`
    );
  }

  const ext = process.platform === 'win32' ? '.exe' : '';
  const binaryPath = `${pkgDir}/bin/cc-switch-server${ext}`;

  return binaryPath;
}
