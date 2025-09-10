/*
 Cross-platform WebSocket creation
 - Browser: uses globalThis.WebSocket
 - Node: dynamically imports 'ws' to avoid bundling it for the browser
*/

export interface WSLike {
  readyState: number;
  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void;
  close(code?: number, reason?: string): void;
  addEventListener(type: string, listener: (ev: any) => void): void;
  removeEventListener(type: string, listener: (ev: any) => void): void;
  // Node 'ws' uses event emitter style; we adapt by mapping
  on?(event: string, listener: (...args: any[]) => void): void;
}

export interface CreateWSOptions {
  compression?: boolean;
}

const isBrowser = typeof globalThis !== 'undefined' && typeof (globalThis as any).window !== 'undefined';

export async function createWebSocket(url: string, opts: CreateWSOptions = {}): Promise<WSLike> {
  if (isBrowser && typeof (globalThis as any).WebSocket !== 'undefined') {
    const ws = new (globalThis as any).WebSocket(url);
    // Ensure on()/addEventListener compatibility
    if (!('on' in ws)) {
      (ws as any).on = (event: string, handler: any) => ws.addEventListener(event, (ev: any) => handler(ev));
    }
    return ws as WSLike;
  }

  // Node path: dynamic import to avoid bundling
  const mod = await import('ws');
  const WSClass = (mod as any).default || (mod as any).WebSocket || mod;
  const ws: any = new WSClass(url, {
    perMessageDeflate: opts.compression ?? true,
  });

  // addEventListener shim for Node ws
  if (!('addEventListener' in ws)) {
    ws.addEventListener = (event: string, listener: any) => ws.on(event, (arg1: any, arg2: any) => {
      // Normalize message signature
      if (event === 'message' && arg1 && arg1.data === undefined) {
        listener({ data: arg1 });
      } else if (event === 'close') {
        listener({ code: arg1, reason: arg2 });
      } else if (event === 'error') {
        listener(arg1);
      } else {
        listener(arg1);
      }
    });
    ws.removeEventListener = (event: string, listener: any) => ws.off?.(event, listener);
  }

  return ws as WSLike;
}

export const ReadyState = {
  CONNECTING: 0,
  OPEN: 1,
  CLOSING: 2,
  CLOSED: 3,
} as const;

