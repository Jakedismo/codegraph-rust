export interface WSLike {
  readyState: number;
  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void;
  close(code?: number, reason?: string): void;
  addEventListener(type: string, listener: (ev: any) => void): void;
  removeEventListener(type: string, listener: (ev: any) => void): void;
}

export async function createBrowserWebSocket(url: string): Promise<WSLike> {
  if (typeof globalThis === 'undefined' || typeof (globalThis as any).WebSocket === 'undefined') {
    throw new Error('WebSocket is not available in this environment');
  }
  const ws = new (globalThis as any).WebSocket(url);
  return ws as WSLike;
}

export const ReadyState = {
  CONNECTING: 0,
  OPEN: 1,
  CLOSING: 2,
  CLOSED: 3,
} as const;

