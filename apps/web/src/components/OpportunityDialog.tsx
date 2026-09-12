import { useEffect, useRef } from 'react';
import type { KeyboardEvent } from 'react';
import { chains, SAMPLE_CAPTURE } from '../domain/fixtures';
import type { Opportunity } from '../domain/fixtures';
import { EvidenceBadge } from './OpportunityTable';

export function OpportunityDialog({ opportunity, onClose }: { opportunity: Opportunity | null; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    const node = dialog.current;
    if (!node || !opportunity) return;
    const previous = document.activeElement;
    if (!node.open) node.showModal();
    return () => {
      if (node.open) node.close();
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  }, [opportunity]);

  function containTab(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key !== 'Tab' || event.ctrlKey || event.altKey || event.metaKey) return;
    const node = event.currentTarget;
    const controls = Array.from(node.querySelectorAll<HTMLElement>(
      'button, a[href], input, select, textarea, [tabindex]',
    )).filter(control => control.tabIndex >= 0
      && !control.matches(':disabled')
      && !control.closest('[inert]')
      && window.getComputedStyle(control).visibility !== 'hidden'
      && control.getClientRects().length > 0);
    const first = controls[0];
    const last = controls.at(-1);
    // showModal makes the background inert, but a browser can still send Tab
    // from the final control to its chrome. Keep keyboard navigation inside
    // the open modal until the operator closes it or presses Escape.
    if (!first || !last) {
      event.preventDefault();
      heading.current?.focus();
    } else if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === node)) {
      event.preventDefault();
      first.focus();
    }
  }

  return <dialog ref={dialog} aria-labelledby="detail-title" onKeyDown={containTab} onCancel={event => { event.preventDefault(); onClose(); }}>
    {opportunity && <>
      <div className="dialoghead"><div><p className="eyebrow">Synthetic opportunity</p><h2 ref={heading} tabIndex={-1} id="detail-title">{opportunity.id} · {chains[opportunity.chain].name}</h2></div><button className="close" onClick={onClose} aria-label="Close opportunity detail" autoFocus>Close</button></div>
      <EvidenceBadge evidence={opportunity.evidence} />
      <p className="detailroute">{opportunity.route}</p><p className="muted">{opportunity.venue} · placeholder pool identities</p>
      <table className="detailcosts"><caption className="sr">Fictional costs and estimates denominated in USDC.</caption><tbody>
        <tr><th scope="row">Input amount</th><td>{opportunity.input}</td></tr>
        <tr><th scope="row">Gross route estimate · USDC</th><td>{opportunity.gross}</td></tr>
        <tr><th scope="row">Other execution costs · USDC</th><td>{opportunity.costs}</td></tr>
        <tr><th scope="row">Estimated net range · USDC</th><td><strong>{opportunity.range}</strong></td></tr>
        <tr><th scope="row">Realized result</th><td>Not applicable — paper demo</td></tr>
      </tbody></table>
      <div className="notice">{opportunity.reason}</div>
      <p className="tiny space-top">All values are synthetic. Capture: {SAMPLE_CAPTURE}. No simulation artifacts are produced by this interface. Pool fees and impact are assumed included in route quotes. Infrastructure and unallocated failures are excluded; native fees are hypothetical USDC valuations. USDC amounts are not guaranteed USD value.</p>
    </>}
  </dialog>;
}
