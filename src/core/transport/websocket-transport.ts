import WebSocket from 'ws';
import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import { JsonRpcMessage, Transport } from '@modelcontextprotocol/sdk/types.js';

export interface WebSocketTransportConfig {
  port?: number;
  host?: string;
  path?: string;
  maxConnections?: number;
  heartbeatInterval?: number;
  reconnectAttempts?: number;
  reconnectDelay?: number;
  messageQueueSize?: number;
  compression?: boolean;
  pingInterval?: number;
  pongTimeout?: number;
}

export interface WebSocketSession {
  id: string;
  socket: WebSocket;
  agentId?: string;
  lastHeartbeat: Date;
  messageQueue: JsonRpcMessage[];
  isAuthenticated: boolean;
  metadata: Record<string, any>;
}

export class WebSocketServerTransport extends EventEmitter implements Transport {
  private server: WebSocket.Server;
  private sessions: Map<string, WebSocketSession> = new Map();
  private config: Required<WebSocketTransportConfig>;
  private heartbeatTimer?: NodeJS.Timer;
  
  constructor(config: WebSocketTransportConfig = {}) {
    super();
    
    this.config = {
      port: config.port ?? 3001,
      host: config.host ?? 'localhost',
      path: config.path ?? '/ws',
      maxConnections: config.maxConnections ?? 1000,
      heartbeatInterval: config.heartbeatInterval ?? 30000,
      reconnectAttempts: config.reconnectAttempts ?? 5,
      reconnectDelay: config.reconnectDelay ?? 1000,
      messageQueueSize: config.messageQueueSize ?? 100,
      compression: config.compression ?? true,
      pingInterval: config.pingInterval ?? 30000,
      pongTimeout: config.pongTimeout ?? 5000
    };

    this.server = new WebSocket.Server({
      port: this.config.port,
      host: this.config.host,
      path: this.config.path,
      perMessageDeflate: this.config.compression,
      maxPayload: 1024 * 1024 * 10 // 10MB max payload
    });

    this.initializeServer();
    this.startHeartbeat();
  }

  private initializeServer(): void {
    this.server.on('connection', this.handleConnection.bind(this));
    this.server.on('error', this.handleServerError.bind(this));
    this.server.on('close', this.handleServerClose.bind(this));
    
    this.emit('server:ready', {
      port: this.config.port,
      host: this.config.host,
      path: this.config.path
    });
  }

  private handleConnection(socket: WebSocket, request: any): void {
    if (this.sessions.size >= this.config.maxConnections) {
      socket.close(1008, 'Maximum connections exceeded');
      return;
    }

    const sessionId = randomUUID();
    const session: WebSocketSession = {
      id: sessionId,
      socket,
      lastHeartbeat: new Date(),
      messageQueue: [],
      isAuthenticated: false,
      metadata: {
        userAgent: request.headers['user-agent'],
        origin: request.headers.origin,
        ip: request.socket.remoteAddress
      }
    };

    this.sessions.set(sessionId, session);

    socket.on('message', (data) => this.handleMessage(sessionId, data));
    socket.on('close', (code, reason) => this.handleDisconnection(sessionId, code, reason));
    socket.on('error', (error) => this.handleSocketError(sessionId, error));
    socket.on('pong', () => this.handlePong(sessionId));

    this.emit('client:connected', { sessionId, session });
    
    // Send welcome message
    this.sendToSession(sessionId, {
      jsonrpc: '2.0',
      method: 'session/initialized',
      params: { sessionId },
      id: null
    });
  }

  private handleMessage(sessionId: string, data: WebSocket.Data): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;

    try {
      const message = JSON.parse(data.toString()) as JsonRpcMessage;
      session.lastHeartbeat = new Date();

      // Validate JSON-RPC format
      if (!this.isValidJsonRpc(message)) {
        this.sendError(sessionId, -32600, 'Invalid Request', null);
        return;
      }

      this.emit('message:received', { sessionId, message, session });
      
      // Handle internal protocol messages
      if (this.isInternalMessage(message)) {
        this.handleInternalMessage(sessionId, message);
      } else {
        this.emit('message', message, sessionId);
      }

    } catch (error) {
      this.sendError(sessionId, -32700, 'Parse error', null);
      this.emit('error', error);
    }
  }

  private handleDisconnection(sessionId: string, code: number, reason: Buffer): void {
    const session = this.sessions.get(sessionId);
    if (session) {
      this.emit('client:disconnected', { 
        sessionId, 
        code, 
        reason: reason.toString(),
        agentId: session.agentId
      });
      this.sessions.delete(sessionId);
    }
  }

  private handleSocketError(sessionId: string, error: Error): void {
    this.emit('socket:error', { sessionId, error });
  }

  private handleServerError(error: Error): void {
    this.emit('server:error', error);
  }

  private handleServerClose(): void {
    this.emit('server:closed');
  }

  private handlePong(sessionId: string): void {
    const session = this.sessions.get(sessionId);
    if (session) {
      session.lastHeartbeat = new Date();
    }
  }

  private isValidJsonRpc(message: any): message is JsonRpcMessage {
    return (
      typeof message === 'object' &&
      message.jsonrpc === '2.0' &&
      typeof message.method === 'string' &&
      (message.id === null || typeof message.id === 'string' || typeof message.id === 'number')
    );
  }

  private isInternalMessage(message: JsonRpcMessage): boolean {
    return message.method.startsWith('transport/') || 
           message.method.startsWith('session/') ||
           message.method === 'ping';
  }

  private handleInternalMessage(sessionId: string, message: JsonRpcMessage): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;

    switch (message.method) {
      case 'ping':
        this.sendToSession(sessionId, {
          jsonrpc: '2.0',
          result: 'pong',
          id: message.id
        });
        break;

      case 'transport/authenticate':
        this.handleAuthentication(sessionId, message);
        break;

      case 'session/register_agent':
        this.handleAgentRegistration(sessionId, message);
        break;

      case 'session/heartbeat':
        session.lastHeartbeat = new Date();
        this.sendToSession(sessionId, {
          jsonrpc: '2.0',
          result: { timestamp: Date.now() },
          id: message.id
        });
        break;
    }
  }

  private handleAuthentication(sessionId: string, message: JsonRpcMessage): void {
    const session = this.sessions.get(sessionId);
    if (!session) return;

    // Authentication logic would be implemented here
    // For now, we'll assume successful authentication
    session.isAuthenticated = true;
    
    this.sendToSession(sessionId, {
      jsonrpc: '2.0',
      result: { authenticated: true },
      id: message.id
    });

    this.emit('client:authenticated', { sessionId, session });
  }

  private handleAgentRegistration(sessionId: string, message: JsonRpcMessage): void {
    const session = this.sessions.get(sessionId);
    if (!session || !session.isAuthenticated) {
      this.sendError(sessionId, -32002, 'Unauthorized', message.id);
      return;
    }

    const params = message.params as any;
    session.agentId = params?.agentId;
    
    this.sendToSession(sessionId, {
      jsonrpc: '2.0',
      result: { registered: true, agentId: session.agentId },
      id: message.id
    });

    this.emit('agent:registered', { sessionId, agentId: session.agentId, session });
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      const now = new Date();
      const timeout = this.config.heartbeatInterval + this.config.pongTimeout;
      
      for (const [sessionId, session] of this.sessions) {
        const timeSinceLastHeartbeat = now.getTime() - session.lastHeartbeat.getTime();
        
        if (timeSinceLastHeartbeat > timeout) {
          session.socket.terminate();
          this.sessions.delete(sessionId);
          this.emit('client:timeout', { sessionId, timeSinceLastHeartbeat });
        } else if (timeSinceLastHeartbeat > this.config.heartbeatInterval) {
          session.socket.ping();
        }
      }
    }, this.config.heartbeatInterval / 2);
  }

  public sendToSession(sessionId: string, message: JsonRpcMessage): boolean {
    const session = this.sessions.get(sessionId);
    if (!session || session.socket.readyState !== WebSocket.OPEN) {
      return false;
    }

    try {
      session.socket.send(JSON.stringify(message));
      return true;
    } catch (error) {
      this.emit('send:error', { sessionId, message, error });
      return false;
    }
  }

  public sendToAgent(agentId: string, message: JsonRpcMessage): boolean {
    for (const [sessionId, session] of this.sessions) {
      if (session.agentId === agentId) {
        return this.sendToSession(sessionId, message);
      }
    }
    return false;
  }

  public broadcast(message: JsonRpcMessage, predicate?: (session: WebSocketSession) => boolean): number {
    let sent = 0;
    for (const [sessionId, session] of this.sessions) {
      if (!predicate || predicate(session)) {
        if (this.sendToSession(sessionId, message)) {
          sent++;
        }
      }
    }
    return sent;
  }

  public broadcastToAgents(agentIds: string[], message: JsonRpcMessage): number {
    return this.broadcast(message, (session) => 
      session.agentId && agentIds.includes(session.agentId)
    );
  }

  private sendError(sessionId: string, code: number, message: string, id: any): void {
    this.sendToSession(sessionId, {
      jsonrpc: '2.0',
      error: { code, message },
      id
    });
  }

  public getSession(sessionId: string): WebSocketSession | undefined {
    return this.sessions.get(sessionId);
  }

  public getSessionsByAgent(agentId: string): WebSocketSession[] {
    return Array.from(this.sessions.values()).filter(s => s.agentId === agentId);
  }

  public getConnectedAgents(): string[] {
    return Array.from(new Set(
      Array.from(this.sessions.values())
        .map(s => s.agentId)
        .filter(Boolean)
    )) as string[];
  }

  public getSessionCount(): number {
    return this.sessions.size;
  }

  public getAgentCount(): number {
    return this.getConnectedAgents().length;
  }

  public async close(): Promise<void> {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
    }

    // Close all client connections
    for (const session of this.sessions.values()) {
      session.socket.close(1001, 'Server shutting down');
    }
    this.sessions.clear();

    // Close server
    return new Promise((resolve) => {
      this.server.close(() => {
        this.emit('server:closed');
        resolve();
      });
    });
  }

  // Transport interface implementation
  public async connect(): Promise<Transport> {
    return this;
  }

  public async read(): Promise<JsonRpcMessage | null> {
    throw new Error('read() not supported on server transport');
  }

  public async write(message: JsonRpcMessage): Promise<void> {
    this.broadcast(message);
  }
}