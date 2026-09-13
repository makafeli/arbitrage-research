import type { FrozenExport } from '../api/frozenExport.ts';
import { MAX_FROZEN_EXPORT_BYTES, parseFrozenExport } from '../api/frozenExport.ts';

export const MAX_FROZEN_CSV_BYTES = 24 * 1024 * 1024;
// Quote every field. Prefix suspicious scalar cells before any whitespace or
// control characters, including Unicode compatibility forms of formula starters.
// payload_json always starts with an object and preserves exact source strings.
export function csvCell(value: string): string {
  const probe = value.normalize('NFKC').replace(/^[\s\p{Cc}\p{Cf}]+/gu, '');
  const protectedValue = /^[=+\-@]/u.test(probe) || /^[\p{Cc}\p{Cf}]/u.test(value) ? "'" + value : value;
  return '"' + protectedValue.replaceAll('"', '""') + '"';
}
export function frozenJson(bundle: FrozenExport): string {
  const result = JSON.stringify(parseFrozenExport(bundle));
  if (new TextEncoder().encode(result).length > MAX_FROZEN_EXPORT_BYTES) throw new Error('The frozen JSON exceeds the 8 MiB download bound.');
  return result;
}
export function frozenCsv(bundle: FrozenExport): string {
  const source = parseFrozenExport(bundle), sessionId = source.data.session.session_id;
  const lines = [['record_type', 'session_id', 'record_id', 'parent_id', 'payload_json'].map(csvCell).join(',')];
  let bytes = new TextEncoder().encode(lines[0] + '\r\n').length;
  function row(type: string, id: string, parent: string, payload: object) {
    const line = [type, sessionId, id, parent, JSON.stringify(payload)].map(csvCell).join(',');
    bytes += new TextEncoder().encode(line + '\r\n').length;
    if (bytes > MAX_FROZEN_CSV_BYTES) throw new Error('The frozen CSV exceeds the 24 MiB download bound. No partial download was prepared.');
    lines.push(line);
  }
  row('EXPORT_MANIFEST', source.export_id, '', { schema_version: source.schema_version, export_id: source.export_id,
    exported_at: source.exported_at, content_sha256: source.content_sha256, snapshot: source.snapshot, methodology: source.methodology,
    csv_schema_version: '1.1.0', csv_notice: 'Each payload_json cell contains a lossless JSON record. Amounts are exact base-unit strings inside JSON. Do not infer decimals. Scalar formula guards may prefix an apostrophe; authoritative identities remain inside payload_json.' });
  row('SESSION', sessionId, '', { session: source.data.session, experiment_id: source.data.experiment_id, strategy_ids: source.data.strategy_ids });
  row('DECISION_COVERAGE', sessionId, '', source.data.decision_coverage);
  for (const item of source.data.decisions) row('DECISION', item.trace.observation_id, sessionId, item);
  for (const item of source.data.paper_runs) {
    row('PAPER_RUN', item.run.run_id, sessionId, item.run);
    for (const event of item.journal) row('PAPER_JOURNAL_EVENT', event.event_id, item.run.run_id, event);
    for (const reservation of item.reservations) row('PAPER_RESERVATION', reservation.attempt_id, item.run.run_id, reservation);
  }
  for (const item of source.data.capture_dependencies) row('CAPTURE_DEPENDENCY', item.capture_id, sessionId, item);
  for (const item of source.data.collection_attempts) row('COLLECTION_ATTEMPT', item.attempt_id, sessionId, item);
  for (const item of source.data.cost_assessments) row('COST_ASSESSMENT', item.record_id, item.assessment.binding.observation_id, item);
  return lines.join('\r\n') + '\r\n';
}
