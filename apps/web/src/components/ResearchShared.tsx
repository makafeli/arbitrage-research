import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import type { Resource } from '../hooks/useResearchResource';
import type { Origin } from '../api/research';
import { researchExport } from '../domain/researchExport';

export function ResourceStatus({ resource }: { resource: Resource<unknown> }) {
  return <>{resource.loading && <p className="notice" role="status">Loading research records…</p>}{resource.error && <div className="notice error-notice" role="alert"><strong>{resource.data ? 'Stale snapshot retained.' : 'Research records unavailable.'}</strong><p>{resource.error}</p></div>}{resource.at !== null && <p className="tiny space-top">Snapshot received {new Date(resource.at).toISOString()}. Explicit refresh; this panel does not claim continuous coverage.</p>}</>;
}
export function Pagination({ label, page, canPrevious, canNext, loading, previous, next }: { label: string; page: number; canPrevious: boolean; canNext: boolean; loading: boolean; previous: () => void; next: () => void }) {
  return <nav className="research-actions space-top" aria-label={label + ' pagination'}><button disabled={loading || !canPrevious} onClick={previous}>Previous {label}</button><span className="tiny">Page {page} · up to 25 records</span><button disabled={loading || !canNext} onClick={next}>Next {label}</button></nav>;
}
export function OriginBadge({ origin }: { origin: Origin }) {
  return <span className={'pill ' + (origin === 'RECORDED_LIVE' ? 'blue' : 'amber')}>{origin === 'SYNTHETIC' ? 'SYNTHETIC DATASET' : origin === 'MANUALLY_CONSTRUCTED' ? 'MANUALLY CONSTRUCTED' : 'RECORDED LIVE INPUT'}</span>;
}
export function ExportButton({ label, scope, data, receivedAt, disabled = false }: { label: string; scope: string; data: unknown; receivedAt: number | null; disabled?: boolean }) {
  const [error, setError] = useState('');
  function download() {
    try {
      const json = researchExport(scope, data, receivedAt);
      const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url; link.download = 'arbitrage-research-' + scope.replace(/[^a-z0-9-]/gi, '-').slice(0, 80) + '.json';
      document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000); setError('');
    } catch (e) { setError(e instanceof Error ? e.message : 'The export could not be prepared.'); }
  }
  return <div><button disabled={disabled || data === null || receivedAt === null} onClick={download}>{label}</button>{error && <p className="notice" role="alert">{error}</p>}</div>;
}
export function ResearchDialog({ open, title, children, close }: { open: boolean; title: string; children: ReactNode; close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null), closeButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open || !dialog.current) return;
    const node = dialog.current, previous = document.activeElement; node.showModal(); closeButton.current?.focus();
    return () => { node.close(); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [open]);
  function trap(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key !== 'Tab' || event.ctrlKey || event.altKey || event.metaKey) return;
    const items = Array.from(dialog.current?.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), [tabindex="0"]') ?? []);
    if (!items.length) return;
    const first = items[0], last = items[items.length - 1];
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }
  return <dialog className="research-dialog" ref={dialog} aria-labelledby="research-detail-title" onKeyDown={trap} onCancel={event => { event.preventDefault(); close(); }}>
    <div className="dialoghead"><h2 id="research-detail-title">{title}</h2><button ref={closeButton} onClick={close}>Close evidence detail</button></div>{open && children}
  </dialog>;
}
export function EmptyResearch({ children }: { children: ReactNode }) { return <p className="notice">{children}</p>; }
export function Exact({ value }: { value: string | null | undefined }) { return <span className="research-exact">{value ?? 'Unknown'}</span>; }
