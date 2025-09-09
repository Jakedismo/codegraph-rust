import WebSocket from 'ws';
import { EventEmitter } from 'events';
import { JsonRpcMessage, Transport } from '@modelcontextprotocol/sdk/types.js';

export interface WebSocketClientConfig {
  url: string;
  reconnectAttempts?: number;
  reconnectDelay?: number;
  heartbeatInterval?: number;
  responseTimeout?: number;
  maxQueueSize?: number;
  compression?: boolean;
}

export class WebSocketClientTransport extends EventEmitter implements Transport {
  private config: Required<WebSocketClientConfig>;
  private socket: WebSocket | null = null;
  private sessionId: string | null = null;
  private isConnected = false;
  private reconnectAttempt = 0;
  private messageQueue: JsonRpcMessage[] = [];
  private pendingResponses = new Map<any, { resolve: Function; reject: Function; timeout: NodeJS.Timeout }>();
  private heartbeatTimer?: NodeJS.Timer;

  constructor(config: WebSocketClientConfig) {
    super();
    
    this.config = {
      url: config.url,
      reconnectAttempts: config.reconnectAttempts ?? 5,
      reconnectDelay: config.reconnectDelay ?? 1000,
      heartbeatInterval: config.heartbeatInterval ?? 30000,
      responseTimeout: config.responseTimeout ?? 30000,
      maxQueueSize: config.maxQueueSize ?? 100,
      compression: config.compression ?? true
    };
  }

  public async connect(): Promise<Transport> {
    return new Promise((resolve, reject) => {
      if (this.isConnected) {
        resolve(this);
        return;
      }

      this.socket = new WebSocket(this.config.url, {
        perMessageDeflate: this.config.compression
      });

      this.socket.on('open', () => {
        this.handleOpen();
        resolve(this);
      });

      this.socket.on('message', this.handleMessage.bind(this));
      this.socket.on('close', this.handleClose.bind(this));
      this.socket.on('error', (error) => {
        this.handleError(error);
        if (!this.isConnected) {
          reject(error);
        }
      });

      this.socket.on('ping', () => this.socket?.pong());
      this.socket.on('pong', () => this.emit('pong'));
    });
  }

  private handleOpen(): void {
    this.isConnected = true;
    this.reconnectAttempt = 0;
    this.startHeartbeat();
    this.flushMessageQueue();
    this.emit('connected');
  }

  private handleMessage(data: WebSocket.Data): void {
    try {
      const message = JSON.parse(data.toString()) as JsonRpcMessage;
      
      // Handle session initialization
      if (message.method === 'session/initialized') {
        this.sessionId = (message.params as any)?.sessionId;
        this.emit('session:initialized', { sessionId: this.sessionId });
        return;
      }

      // Handle responses to pending requests
      if (message.id !== undefined && this.pendingResponses.has(message.id)) {
        const pending = this.pendingResponses.get(message.id)!;
        clearTimeout(pending.timeout);
        this.pendingResponses.delete(message.id);
        
        if ('error' in message) {
          pending.reject(new Error(message.error?.message || 'Request failed'));
        } else {
          pending.resolve(message);
        }
        return;
      }

      // Emit regular messages
      this.emit('message', message);
      
    } catch (error) {
      this.emit('error', new Error(`Failed to parse message: ${error}`));
    }
  }

  private handleClose(code: number, reason: Buffer): void {
    this.isConnected = false;
    this.stopHeartbeat();
    
    // Clear pending responses
    for (const pending of this.pendingResponses.values()) {
      clearTimeout(pending.timeout);
      pending.reject(new Error('Connection closed'));
    }
    this.pendingResponses.clear();
    
    this.emit('disconnected', { code, reason: reason.toString() });
    
    // Attempt reconnection
    if (this.reconnectAttempt < this.config.reconnectAttempts) {
      this.attemptReconnection();
    } else {
      this.emit('connection:failed', { attempts: this.reconnectAttempt });
    }
  }

  private handleError(error: Error): void {
    this.emit('error', error);
  }

  private attemptReconnection(): void {
    this.reconnectAttempt++;
    const delay = this.config.reconnectDelay * Math.pow(2, this.reconnectAttempt - 1);
    
    this.emit('reconnecting', { attempt: this.reconnectAttempt, delay });
    
    setTimeout(async () => {
      try {
        await this.connect();
      } catch (error) {
        // Connection will handle retry logic
      }
    }, delay);
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (this.isConnected && this.socket?.readyState === WebSocket.OPEN) {
        this.socket.ping();
      }
    }, this.config.heartbeatInterval);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
  }

  private flushMessageQueue(): void {
    while (this.messageQueue.length > 0 && this.isConnected) {
      const message = this.messageQueue.shift()!;
      this.sendMessage(message);
    }
  }

  private sendMessage(message: JsonRpcMessage): void {
    if (!this.isConnected || !this.socket || this.socket.readyState !== WebSocket.OPEN) {
      if (this.messageQueue.length < this.config.maxQueueSize) {
        this.messageQueue.push(message);
      } else {
        this.emit('queue:overflow', { message });
      }
      return;
    }

    try {
      this.socket.send(JSON.stringify(message));
      this.emit('message:sent', { message });
    } catch (error) {
      this.emit('send:error', { message, error });
    }
  }

  public async read(): Promise<JsonRpcMessage | null> {
    return new Promise((resolve) => {
      const handler = (message: JsonRpcMessage) => {
        this.removeListener('message', handler);
        resolve(message);
      };
      this.once('message', handler);
    });
  }

  public async write(message: JsonRpcMessage): Promise<void> {
    this.sendMessage(message);
  }

  public async request(message: JsonRpcMessage): Promise<JsonRpcMessage> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingResponses.delete(message.id);
        reject(new Error(`Request timeout after ${this.config.responseTimeout}ms`));
      }, this.config.responseTimeout);

      this.pendingResponses.set(message.id, { resolve, reject, timeout });
      this.sendMessage(message);
    });
  }

  public async authenticate(credentials: any): Promise<boolean> {
    try {
      const response = await this.request({
        jsonrpc: '2.0',
        method: 'transport/authenticate',
        params: credentials,
        id: this.generateRequestId()
      });

      return (response.result as any)?.authenticated === true;
    } catch (error) {
      this.emit('auth:error', { error });
      return false;
    }
  }

  public async registerAgent(agentId: string, metadata?: any): Promise<boolean> {
    try {
      const response = await this.request({
        jsonrpc: '2.0',
        method: 'session/register_agent',
        params: { agentId, metadata },
        id: this.generateRequestId()
      });

      const registered = (response.result as any)?.registered === true;
      if (registered) {
        this.emit('agent:registered', { agentId });
      }
      return registered;
    } catch (error) {
      this.emit('agent:registration_error', { agentId, error });
      return false;
    }
  }

  public async ping(): Promise<number> {
    const start = Date.now();
    try {
      await this.request({
        jsonrpc: '2.0',
        method: 'ping',
        params: {},
        id: this.generateRequestId()
      });
      return Date.now() - start;
    } catch (error) {
      throw new Error(`Ping failed: ${error}`);
    }
  }

  private generateRequestId(): string {
    return Math.random().toString(36).substr(2, 9);
  }

  public getSessionId(): string | null {
    return this.sessionId;
  }

  public isReady(): boolean {
    return this.isConnected && this.socket?.readyState === WebSocket.OPEN;
  }

  public getQueueSize(): number {
    return this.messageQueue.length;
  }

  public getPendingRequests(): number {
    return this.pendingResponses.size;
  }

  public async close(): Promise<void> {
    this.stopHeartbeat();
    
    // Clear pending responses
    for (const pending of this.pendingResponses.values()) {
      clearTimeout(pending.timeout);
      pending.reject(new Error('Transport closed'));
    }
    this.pendingResponses.clear();
    
    // Clear message queue
    this.messageQueue = [];
    
    if (this.socket) {
      return new Promise((resolve) => {
        if (this.socket?.readyState === WebSocket.OPEN) {
          this.socket.close(1000, 'Normal closure');
          this.socket.once('close', () => resolve());
        } else {
          resolve();
        }
      });
    }
  }
}