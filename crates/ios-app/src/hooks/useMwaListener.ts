import { useEffect, useState } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// Must match the Rust MwaPendingRequest struct
export interface MwaPendingRequest {
  id: number;
  request_id: string;
  request: {
    method: string;
    params: unknown;
  };
}

export function useMwaListener() {
  const [currentRequest, setCurrentRequest] = useState<MwaPendingRequest | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    listen<MwaPendingRequest>("mwa://request", (event) => {
      setCurrentRequest(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  function clearRequest() {
    setCurrentRequest(null);
  }

  return { currentRequest, clearRequest };
}
