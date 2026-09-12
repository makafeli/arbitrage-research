import { useEffect, useRef, useState } from 'react';

export interface Resource<T> { data: T | null; at: number | null; error: string | null; loading: boolean; refresh: () => void }
export function useResearchResource<T>(active: boolean, scope: string, loader: (signal: AbortSignal) => Promise<T>): Resource<T> {
  const load = useRef(loader); load.current = loader;
  const [version, setVersion] = useState(0);
  const [state, setState] = useState<{ scope: string; data: T | null; at: number | null; error: string | null; loading: boolean }>({ scope: '', data: null, at: null, error: null, loading: false });
  useEffect(() => {
    if (!active || !scope) return;
    let current = true;
    const controller = new AbortController();
    setState(previous => ({ scope, data: previous.scope === scope ? previous.data : null, at: previous.scope === scope ? previous.at : null, error: null, loading: true }));
    void load.current(controller.signal).then(data => {
      if (current) setState({ scope, data, at: Date.now(), error: null, loading: false });
    }).catch(() => {
      if (current && !controller.signal.aborted) setState(previous => ({ ...previous, error: 'Research data could not be refreshed. Check the service connection and sign in again if the session expired.', loading: false }));
    });
    return () => { current = false; controller.abort(); };
  }, [active, scope, version]);
  return { data: state.scope === scope ? state.data : null, at: state.scope === scope ? state.at : null,
    error: state.scope === scope ? state.error : null, loading: active && Boolean(scope) && (state.scope !== scope || state.loading),
    refresh: () => setVersion(value => value + 1) };
}
export function useResearchPagination(scope: string) {
  const [state, setState] = useState<{ scope: string; cursors: (string | undefined)[] }>({ scope, cursors: [undefined] });
  const cursors = state.scope === scope ? state.cursors : [undefined];
  return {
    cursor: cursors[cursors.length - 1], page: cursors.length, canPrevious: cursors.length > 1,
    canNext: (next: string | null | undefined) => Boolean(next && !cursors.includes(next) && cursors.length < 100),
    previous: () => setState({ scope, cursors: cursors.slice(0, -1).length ? cursors.slice(0, -1) : [undefined] }),
    next: (next: string) => { if (!cursors.includes(next) && cursors.length < 100) setState({ scope, cursors: [...cursors, next] }); },
    reset: () => setState({ scope, cursors: [undefined] }),
  };
}
