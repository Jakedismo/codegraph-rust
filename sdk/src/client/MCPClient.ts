import { createWebSocket, ReadyState, type WSLike } from '../transport/universal-websocket.js';
import type { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';
import {
  MCPClientOptions,
  MCPClientEvents,
  EventKey,
  JsonRpcId,
  MCPRequest,
} from '../types.js';

type Listener<K extends EventKey> = MCPClientEvents[K];

export class MCPClient {
  private opts: Required<MCPClientOptions>;
  private ws: WSLike | null = null;
  private connected = false;
  private reconnectAttempt = 0;
  private sessionId: string | null = null;
  private pending = new Map<JsonRpcId, { resolve: (msg: JsonRpcMessage) => void; reject: (err: Error) => void; timer: any }>();
  private listeners: { [K in EventKey]?: Set<Listener<K>> } = {} as any;

  constructor(options: MCPClientOptions) {
    this.opts = {
      capabilities: options.capabilities ?? {},
      agent: options.agent,
      auth: options.auth,
      url: options.url,
      reconnectAttempts: options.reconnectAttempts ?? 5,
      reconnectDelayMs: options.reconnectDelayMs ?? 1000,
      responseTimeoutMs: options.responseTimeoutMs ?? 30000,
      heartbeatIntervalMs: options.heartbeatIntervalMs ?? 30000,
      compression: options.compression ?? true,
    };
  }

  public on<K extends EventKey>(event: K, listener: Listener<K>) { (this.listeners[event] ||= new Set()).add(listener as any); return { dispose: () => this.off(event, listener) }; }
  public off<K extends EventKey>(event: K, listener: Listener<K>) { this.listeners[event]?.delete(listener as any); }
  private emit<K extends EventKey>(event: K, ...args: Parameters<MCPClientEvents[K]>) { this.listeners[event]?.forEach(l => (l as any)(...args)); }

  public isConnected() { return this.connected; }
  public getSessionId() { return this.sessionId; }

  public async connect(): Promise<void> {
    if (this.connected) return;
    await this.openSocket();
  }

  private async openSocket(): Promise<void> {
    this.ws = await createWebSocket(this.opts.url, { compression: this.opts.compression });

    this.ws.addEventListener('open', () => {
      this.connected = true;
      this.reconnectAttempt = 0;
      this.emit('open');
    });

    this.ws.addEventListener('message', (evt: MessageEvent) => {
      let message: JsonRpcMessage | null = null;
      try {
        message = JSON.parse((evt as any).data?.toString?.() ?? (evt as any).data) as JsonRpcMessage;
      } catch (e) {
        this.emit('error', new Error('Failed to parse message'));
        return;
      }

      // Session initialization from server
      if (message.method === 'session/initialized') {
        this.sessionId = (message.params as any)?.sessionId ?? null;
        this.emit('session', { sessionId: this.sessionId! });
        return;
      }

      // Resolve pending requests
      if (message && (message as any).id != null && this.pending.has((message as any).id)) {
        const p = this.pending.get((message as any).id)!;
        clearTimeout(p.timer);
        this.pending.delete((message as any).id);
        if ((message as any).error) {
          p.reject(new Error((message as any).error?.message ?? 'Request failed'));
        } else {
          p.resolve(message);
        }
        return;
      }

      this.emit('message', message);
    });

    this.ws.addEventListener('close', (ev: any) => {
      const code = (ev as any).code;
      const reason = (ev as any).reason?.toString?.() ?? undefined;
      this.connected = false;
      this.emit('close', { code, reason });
      // Reject all pending
      for (const [id, p] of this.pending) {
        clearTimeout(p.timer);
        p.reject(new Error('Connection closed'));
        this.pending.delete(id);
      }
      this.tryReconnect();
    });

    this.ws.addEventListener('error', (err: any) => {
      const e = err instanceof Error ? err : new Error('WebSocket error');
      this.emit('error', e);
    });
  }

  private tryReconnect() {
    if (this.reconnectAttempt >= this.opts.reconnectAttempts) return;
    this.reconnectAttempt++;
    const backoff = this.opts.reconnectDelayMs * Math.pow(2, this.reconnectAttempt - 1);
    setTimeout(async () => {
      try {
        await this.openSocket();
        this.emit('reconnected', { attempt: this.reconnectAttempt });
        // Re-auth and re-register if needed
        await this.postConnectHandshake();
      } catch {
        this.tryReconnect();
      }
    }, backoff);
  }

  private newId(): string { return Math.random().toString(36).slice(2); }

  private async postConnectHandshake(): Promise<void> {
    // Authenticate if configured
    if (this.opts.auth) {
      await this.request('transport/authenticate', this.opts.auth);
    }

    // Register agent if configured
    if (this.opts.agent?.agentId) {
      await this.request('session/register_agent', {
        agentId: this.opts.agent.agentId,
        metadata: this.opts.agent.metadata ?? {},
      });
    }

    // Optional: send initialize per MCP schema (server may ignore)
    await this.notify('initialize', {
      protocolVersion: '2024-08-28',
      capabilities: this.opts.capabilities ?? {},
      clientInfo: { name: 'codegraph-mcp-js-sdk', version: '0.1.0' },
    });
  }

  public async handshake(): Promise<void> {
    await this.connect();
    await this.postConnectHandshake();
  }

  public async notify<TParams = unknown>(method: string, params?: TParams): Promise<void> {
    this.ensureOpen();
    const msg: JsonRpcMessage = { jsonrpc: '2.0', method, params, id: null } as any;
    (this.ws as WSLike).send(JSON.stringify(msg));
  }

  public async request<TParams = unknown>(method: string, params?: TParams): Promise<JsonRpcMessage> {
    this.ensureOpen();
    const id = this.newId();
    const msg: JsonRpcMessage = { jsonrpc: '2.0', method, params, id } as any;

    return new Promise<JsonRpcMessage>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Request timeout after ${this.opts.responseTimeoutMs}ms`));
      }, this.opts.responseTimeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      (this.ws as WSLike).send(JSON.stringify(msg));
    });
  }

  public async call<TParams = unknown>(req: MCPRequest<TParams>): Promise<JsonRpcMessage> {
    return this.request(req.method, req.params);
  }

  public async ping(): Promise<number> {
    const start = Date.now();
    await this.request('ping', {});
    return Date.now() - start;
  }

  public async close(code?: number, reason?: string): Promise<void> {
    if (!this.ws) return;
    try { this.ws.close(code, reason); } catch {}
    this.ws = null;
    this.connected = false;
  }

  private ensureOpen() {
    if (!this.ws || this.ws.readyState !== ReadyState.OPEN) {
      throw new Error('WebSocket is not open');
    }
  }
}

export default MCPClient;

