import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

export function useEvent<T>(name: string, cb: (payload: T) => void) {
  const ref = useRef(cb);
  ref.current = cb;
  useEffect(() => {
    const p = listen<T>(name, (e) => ref.current(e.payload));
    return () => {
      p.then((off) => off());
    };
  }, [name]);
}
