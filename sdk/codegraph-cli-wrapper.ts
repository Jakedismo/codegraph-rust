/**
 * TypeScript wrapper for CodeGraph CLI
 * Spawns the Rust CLI binary and parses JSON output
 */

import { spawn, SpawnOptions } from 'child_process';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';

const exec = promisify(execCallback);

export interface CodeGraphOptions {
  /**
   * Path to the codegraph binary
   * @default 'codegraph' (searches PATH)
   */
  binaryPath?: string;

  /**
   * Storage path for CodeGraph data
   * @default undefined (uses default ~/.codegraph)
   */
  storagePath?: string;

  /**
   * Enable verbose output
   * @default false
   */
  verbose?: boolean;

  /**
   * Timeout for commands in milliseconds
   * @default 30000 (30 seconds)
   */
  timeout?: number;
}

export interface TransactionResult {
  transaction_id: string;
  isolation_level: string;
  status: string;
}

export interface VersionResult {
  version_id: string;
  name: string;
  description: string;
  author: string;
  created_at: string;
}

export interface BranchResult {
  name: string;
  head: string;
  created_at: string;
  created_by: string;
}

export interface TransactionStats {
  active_transactions: number;
  committed_transactions: number;
  aborted_transactions: number;
  average_commit_time_ms: number;
}

export interface MergeResult {
  source: string;
  target: string;
  author: string;
  message?: string;
  success: boolean;
  conflicts: number;
  merged_version_id?: string;
}

export type IsolationLevel = 'read-uncommitted' | 'read-committed' | 'repeatable-read' | 'serializable';

/**
 * CodeGraph CLI wrapper
 */
export class CodeGraph {
  private binaryPath: string;
  private storagePath?: string;
  private verbose: boolean;
  private timeout: number;

  constructor(options: CodeGraphOptions = {}) {
    this.binaryPath = options.binaryPath || 'codegraph';
    this.storagePath = options.storagePath;
    this.verbose = options.verbose || false;
    this.timeout = options.timeout || 30000;
  }

  /**
   * Execute a CLI command and return parsed JSON output
   */
  private async run<T>(args: string[]): Promise<T> {
    const fullArgs = ['--output', 'json'];

    if (this.storagePath) {
      fullArgs.push('--storage', this.storagePath);
    }

    if (this.verbose) {
      fullArgs.push('--verbose');
    }

    fullArgs.push(...args);

    return new Promise((resolve, reject) => {
      const proc = spawn(this.binaryPath, fullArgs, {
        stdio: ['ignore', 'pipe', 'pipe'],
      });

      let stdout = '';
      let stderr = '';

      const timer = setTimeout(() => {
        proc.kill('SIGTERM');
        reject(new Error(`Command timed out after ${this.timeout}ms`));
      }, this.timeout);

      proc.stdout?.on('data', (data) => {
        stdout += data.toString();
      });

      proc.stderr?.on('data', (data) => {
        stderr += data.toString();
      });

      proc.on('close', (code) => {
        clearTimeout(timer);

        if (code !== 0) {
          reject(new Error(`Command failed with code ${code}: ${stderr}`));
          return;
        }

        try {
          const result = JSON.parse(stdout);
          resolve(result);
        } catch (e) {
          reject(new Error(`Failed to parse JSON output: ${stdout}`));
        }
      });

      proc.on('error', (err) => {
        clearTimeout(timer);
        reject(new Error(`Failed to spawn codegraph binary: ${err.message}`));
      });
    });
  }

  // ========================================
  // Transaction Management
  // ========================================

  /**
   * Begin a new transaction
   */
  async beginTransaction(isolationLevel: IsolationLevel = 'read-committed'): Promise<TransactionResult> {
    return this.run<TransactionResult>([
      'transaction',
      'begin',
      '--isolation',
      isolationLevel,
    ]);
  }

  /**
   * Commit a transaction
   */
  async commitTransaction(transactionId: string): Promise<TransactionResult> {
    return this.run<TransactionResult>([
      'transaction',
      'commit',
      transactionId,
    ]);
  }

  /**
   * Rollback a transaction
   */
  async rollbackTransaction(transactionId: string): Promise<TransactionResult> {
    return this.run<TransactionResult>([
      'transaction',
      'rollback',
      transactionId,
    ]);
  }

  /**
   * Get transaction statistics
   */
  async getTransactionStats(): Promise<TransactionStats> {
    return this.run<TransactionStats>(['transaction', 'stats']);
  }

  // ========================================
  // Version Management
  // ========================================

  /**
   * Create a new version
   */
  async createVersion(params: {
    name: string;
    description: string;
    author: string;
    parents?: string[];
  }): Promise<VersionResult> {
    const args = [
      'version',
      'create',
      '--name',
      params.name,
      '--description',
      params.description,
      '--author',
      params.author,
    ];

    if (params.parents && params.parents.length > 0) {
      args.push('--parents', params.parents.join(','));
    }

    return this.run<VersionResult>(args);
  }

  /**
   * List versions
   */
  async listVersions(limit: number = 50): Promise<VersionResult[]> {
    return this.run<VersionResult[]>([
      'version',
      'list',
      '--limit',
      limit.toString(),
    ]);
  }

  /**
   * Get version details
   */
  async getVersion(versionId: string): Promise<VersionResult> {
    return this.run<VersionResult>(['version', 'get', versionId]);
  }

  /**
   * Tag a version
   */
  async tagVersion(params: {
    versionId: string;
    tag: string;
    author: string;
    message?: string;
  }): Promise<{ version_id: string; tag: string; status: string }> {
    const args = [
      'version',
      'tag',
      params.versionId,
      '--tag',
      params.tag,
      '--author',
      params.author,
    ];

    if (params.message) {
      args.push('--message', params.message);
    }

    return this.run(args);
  }

  /**
   * Compare two versions
   */
  async compareVersions(fromVersion: string, toVersion: string): Promise<{
    from_version: string;
    to_version: string;
    added_nodes: number;
    modified_nodes: number;
    deleted_nodes: number;
  }> {
    return this.run(['version', 'compare', fromVersion, toVersion]);
  }

  // ========================================
  // Branch Management
  // ========================================

  /**
   * Create a new branch
   */
  async createBranch(params: {
    name: string;
    from: string;
    author: string;
    description?: string;
  }): Promise<BranchResult> {
    const args = [
      'branch',
      'create',
      '--name',
      params.name,
      '--from',
      params.from,
      '--author',
      params.author,
    ];

    if (params.description) {
      args.push('--description', params.description);
    }

    return this.run<BranchResult>(args);
  }

  /**
   * List branches
   */
  async listBranches(): Promise<BranchResult[]> {
    return this.run<BranchResult[]>(['branch', 'list']);
  }

  /**
   * Get branch details
   */
  async getBranch(name: string): Promise<BranchResult> {
    return this.run<BranchResult>(['branch', 'get', name]);
  }

  /**
   * Delete a branch
   */
  async deleteBranch(name: string): Promise<{ name: string; status: string }> {
    return this.run(['branch', 'delete', name]);
  }

  /**
   * Merge branches
   */
  async mergeBranches(params: {
    source: string;
    target: string;
    author: string;
    message?: string;
  }): Promise<MergeResult> {
    const args = [
      'branch',
      'merge',
      '--source',
      params.source,
      '--target',
      params.target,
      '--author',
      params.author,
    ];

    if (params.message) {
      args.push('--message', params.message);
    }

    return this.run<MergeResult>(args);
  }

  // ========================================
  // System
  // ========================================

  /**
   * Get system status
   */
  async status(): Promise<{
    storage_path: string;
    status: string;
    message: string;
  }> {
    return this.run(['status']);
  }

  /**
   * Check if the codegraph binary is available
   */
  async checkBinary(): Promise<boolean> {
    try {
      const { stdout } = await exec(`${this.binaryPath} --version`);
      return stdout.includes('codegraph');
    } catch {
      return false;
    }
  }
}

// ========================================
// Convenience exports
// ========================================

/**
 * Create a CodeGraph client with default options
 */
export function createCodeGraphClient(options?: CodeGraphOptions): CodeGraph {
  return new CodeGraph(options);
}

/**
 * Default export
 */
export default CodeGraph;
