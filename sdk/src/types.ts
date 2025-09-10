import type { JsonRpcMessage } from '@modelcontextprotocol/sdk/types.js';

export type JSONValue = string | number | boolean | null | JSONValue[] | { [k: string]: JSONValue };

export interface MCPClientCapabilities {
  resources?: Record<string, unknown>;
  tools?: Record<string, unknown>;
  prompts?: Record<string, unknown>;
  sampling?: Record<string, unknown>;
  roots?: Record<string, unknown>;
}

export interface MCPClientAuth {
  // Arbitrary credentials payload understood by the server
  [key: string]: JSONValue;
}

export interface AgentRegistrationMetadata {
  [key: string]: JSONValue;
}

export interface AgentRegistration {
  agentId: string;
  metadata?: AgentRegistrationMetadata;
}

export interface MCPClientOptions {
  url: string;
  capabilities?: MCPClientCapabilities;
  auth?: MCPClientAuth;
  agent?: AgentRegistration;
  reconnectAttempts?: number;
  reconnectDelayMs?: number;
  responseTimeoutMs?: number;
  heartbeatIntervalMs?: number;
  compression?: boolean; // node-only ws permessage-deflate
}

export interface MCPRequest<TParams = unknown> {
  method: string;
  params?: TParams;
}

export interface MCPClientEvents {
  open: () => void;
  close: (info: { code?: number; reason?: string }) => void;
  error: (err: Error) => void;
  message: (msg: JsonRpcMessage) => void;
  reconnected: (info: { attempt: number }) => void;
  session: (info: { sessionId: string }) => void;
}

export type EventKey = keyof MCPClientEvents;

export interface Disposable {
  dispose(): void;
}

export type JsonRpcId = string | number;

export { JsonRpcMessage };

