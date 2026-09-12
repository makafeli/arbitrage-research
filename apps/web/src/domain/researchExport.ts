export const MAX_EXPORT_BYTES = 2 * 1024 * 1024;
export function researchExport(scope: string, data: unknown, receivedAt: number | null, now = Date.now()): string {
  if (receivedAt === null) throw new Error('No received snapshot is available to export.');
  const json = JSON.stringify({
    export_schema_version: '1.0.0', scope, snapshot_received_at: new Date(receivedAt).toISOString(),
    exported_at: new Date(now).toISOString(), evidence_notice: 'Research evidence only. Paper accounting is hypothetical. Quotes do not establish executable or realized profit.',
    collection_notice: 'This export contains only the selected received data. It does not establish complete collection or an atomic cross-resource snapshot.',
    retention_notice: 'Capture references do not establish retained raw artifacts. Journal exports include command kind and postings, not command inputs.',
    data,
  }, null, 2);
  if (new TextEncoder().encode(json).length > MAX_EXPORT_BYTES) throw new Error('This selection exceeds the 2 MiB export limit. Export a smaller page or one record.');
  return json;
}
