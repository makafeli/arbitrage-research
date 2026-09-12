import { chains, SAMPLE_CAPTURE } from '../domain/fixtures';
import type { Opportunity, DemoEvidence } from '../domain/fixtures';

export function EvidenceBadge({ evidence }: { evidence: DemoEvidence }) {
  const tone = evidence === 'Estimated executable' ? 'paper' : evidence === 'Simulated' ? 'blue' : '';
  return <span className={`pill ${tone}`}>{evidence}</span>;
}

export function OpportunityTable({ data, onInspect }: { data: readonly Opportunity[]; onInspect: (opportunity: Opportunity) => void }) {
  return <div className="tablewrap">
    <table>
      <caption className="sr">Synthetic opportunities. Net ranges are fictional USDC amounts, never realized returns.</caption>
      <thead><tr><th scope="col">Route / chain</th><th scope="col">Evidence</th><th scope="col" className="num">Input size</th><th scope="col" className="num">Estimated net · USDC</th><th scope="col">Quality at capture</th><th scope="col"><span className="sr">Details</span></th></tr></thead>
      <tbody>{data.map(item => <tr key={item.id}>
        <td><span className="route">{item.route}</span><span className="route-sub">{chains[item.chain].name} · {item.id} · synthetic</span></td>
        <td><EvidenceBadge evidence={item.evidence} /></td>
        <td className="num"><span className="mobilelabel">Input size</span>{item.input}</td>
        <td className="num"><span className="mobilelabel">Estimated net · USDC · synthetic</span><span className="net">{item.range}</span></td>
        <td><span className="tiny">{item.quality}</span></td>
        <td><button className="cellbtn" onClick={() => onInspect(item)} aria-label={`Inspect synthetic opportunity ${item.id}`}>Inspect</button></td>
      </tr>)}</tbody>
    </table>
    <div className="tablefoot">Illustrative capture: {SAMPLE_CAPTURE}. Pool identities are placeholders. Quotes include pool fees and impact; additional costs are separate assumptions.</div>
  </div>;
}
