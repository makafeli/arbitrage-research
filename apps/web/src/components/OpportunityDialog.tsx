import { useEffect, useRef } from 'react';
import { chains, SAMPLE_CAPTURE } from '../domain/fixtures';
import type { Opportunity } from '../domain/fixtures';
import { EvidenceBadge } from './OpportunityTable';

export function OpportunityDialog({ opportunity, onClose }: { opportunity: Opportunity | null; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
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

  return <dialog ref={dialog} aria-labelledby="detail-title" onCancel={event => { event.preventDefault(); onClose(); }}>
    {opportunity && <>
      <div className="dialoghead"><div><p className="eyebrow">Synthetic opportunity</p><h2 id="detail-title">{opportunity.id} · {chains[opportunity.chain].name}</h2></div><button className="close" onClick={onClose} aria-label="Close opportunity detail" autoFocus>Close</button></div>
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
