import test from 'node:test';
import assert from 'node:assert/strict';
import { initialSessions, pendingTransition, selectChains, transition } from '../src/domain/lifecycle.ts';
import { opportunities } from '../src/domain/fixtures.ts';

test('each demo session belongs to a single network; a view filter cannot mutate run scope', () => {
  const sessions = initialSessions();
  assert.deepEqual(selectChains('base'), ['base']);
  assert.deepEqual(sessions.map(session => session.chain), ['solana', 'base']);
  assert.ok(sessions.every(session => session.state === 'STOPPED' && session.commandStatus === 'NONE'));
});

test('STOP acceptance is PENDING and independent acknowledgments cannot stop another session', () => {
  const running = initialSessions().map(session => transition(session, 'START'));
  const requested = running.map(session => transition(session, 'STOP'));
  assert.ok(requested.every(session => session.state === 'PAUSING' && session.commandStatus === 'PENDING'));
  const solanaAcknowledged = [transition(requested[0], 'ACK_STOP'), requested[1]];
  assert.equal(solanaAcknowledged[0].state, 'STOPPED');
  assert.equal(solanaAcknowledged[0].commandStatus, 'APPLIED');
  assert.equal(solanaAcknowledged[1].state, 'PAUSING');
  assert.equal(solanaAcknowledged[1].commandStatus, 'PENDING');
});

test('start is rejected during stop pending; only an acknowledgment completes stop', () => {
  const requested = transition(transition(initialSessions()[0], 'START'), 'STOP');
  assert.equal(transition(requested, 'START'), requested);
  assert.equal(transition(requested, 'PAUSE'), requested);
  assert.equal(transition(requested, 'STOP'), requested);
  assert.equal(transition(requested, 'ACK_STOP').state, 'STOPPED');
});

test('pause permits resume or stop and cannot start from a stale stop acknowledgment', () => {
  const paused = transition(transition(initialSessions()[0], 'START'), 'PAUSE');
  assert.equal(paused.state, 'PAUSED');
  assert.equal(transition(paused, 'START').state, 'RUNNING');
  assert.equal(transition(paused, 'STOP').state, 'PAUSING');
  assert.equal(transition(paused, 'ACK_STOP'), paused);
});

test('an applied fence with an unresolved outcome stays DRAINING until explicit reconciliation', () => {
  assert.equal(pendingTransition('INACTIVE', 'RESOLVE'), 'INACTIVE');
  const pending = pendingTransition('INACTIVE', 'SIMULATE');
  assert.equal(pending, 'DRAINING');
  assert.equal(pendingTransition(pending, 'SIMULATE'), 'DRAINING');
  assert.equal(pendingTransition(pending, 'RESOLVE'), 'STOPPED');
});

test('synthetic examples cover both chains and never present paper results as Realized', () => {
  assert.equal(new Set(opportunities.map(item => item.id)).size, opportunities.length);
  assert.deepEqual([...new Set(opportunities.map(item => item.chain))].sort(), ['base', 'solana']);
  assert.ok(opportunities.every(item => ['Candidate', 'Simulated', 'Estimated executable'].includes(item.evidence)));
  assert.ok(opportunities.filter(item => item.evidence === 'Candidate').every(item => item.costs === 'Unknown'));
});
