import { useEffect, useRef, useCallback, useState } from "react";
import { getWsBase, getAuthToken } from "@/lib/api";

interface UseWebSocketOptions {
  path: string;
  onMessage?: (data: any) => void;
  onOpen?: () => void;
  onClose?: () => void;
  reconnectInterval?: number;
  enabled?: boolean;
}

export function useWebSocket({
  path,
  onMessage,
  onOpen,
  onClose,
  reconnectInterval = 3000,
  enabled = true,
}: UseWebSocketOptions) {
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout>>();
  const onMessageRef = useRef(onMessage);
  const onOpenRef = useRef(onOpen);
  const onCloseRef = useRef(onClose);

  onMessageRef.current = onMessage;
  onOpenRef.current = onOpen;
  onCloseRef.current = onClose;

  const connect = useCallback(() => {
    if (!enabled) return;

    const token = getAuthToken();
    const wsUrl = `${getWsBase()}${path}${token ? `?token=${token}` : ""}`;

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setIsConnected(true);
      onOpenRef.current?.();
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        onMessageRef.current?.(data);
      } catch {
        onMessageRef.current?.(event.data);
      }
    };

    ws.onclose = () => {
      setIsConnected(false);
      onCloseRef.current?.();
      reconnectTimer.current = setTimeout(connect, reconnectInterval);
    };

    ws.onerror = () => {
      ws.close();
    };
  }, [path, enabled, reconnectInterval]);

  useEffect(() => {
    connect();
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [connect]);

  const send = useCallback((data: any) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(typeof data === "string" ? data : JSON.stringify(data));
    }
  }, []);

  return { isConnected, send, ws: wsRef };
}
