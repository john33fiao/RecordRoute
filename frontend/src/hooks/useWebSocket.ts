import { useEffect, useRef, useCallback } from 'react';
import { resolveWebSocketUrl } from '../runtime/endpoints';

type MessageHandler = (
  taskId: string,
  message: string,
  meta?: { stage?: 'upload' | 'transform' | 'correct' | 'summary'; error_code?: string; retryable?: boolean; failed_step?: string; progress_percent?: number; eta_seconds?: number | null; error?: { message: string; code?: string | null; retryable?: boolean | null; failed_step?: string | null } | null }
) => void;

export function useWebSocket(onMessage: MessageHandler) {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const onMessageRef = useRef(onMessage);
  onMessageRef.current = onMessage;

  const connect = useCallback(() => {
    try {
      const wsUrl = resolveWebSocketUrl();
      const ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        console.log('WebSocket connected');
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.task_id && data.message) {
            onMessageRef.current(data.task_id, data.message, {
              stage: data.stage,
              error_code: data.error_code,
              retryable: data.retryable,
              failed_step: data.failed_step,
              progress_percent: data.progress_percent,
              eta_seconds: data.eta_seconds,
              error: data.error,
            });
          }
        } catch (e) {
          console.error('WebSocket message parse error:', e);
        }
      };

      ws.onclose = () => {
        console.log('WebSocket disconnected, reconnecting...');
        reconnectTimerRef.current = setTimeout(connect, 3000);
      };

      ws.onerror = () => {
        ws.close();
      };

      wsRef.current = ws;
    } catch (e) {
      console.error('WebSocket connection error:', e);
      reconnectTimerRef.current = setTimeout(connect, 3000);
    }
  }, []);

  useEffect(() => {
    connect();
    return () => {
      clearTimeout(reconnectTimerRef.current);
      wsRef.current?.close();
    };
  }, [connect]);
}
