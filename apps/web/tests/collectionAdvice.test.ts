import assert from 'node:assert/strict';
import test from 'node:test';
import { parseCollectionAttempt } from '../src/api/collection.ts';
import { advice, attemptLabel, attemptTone } from '../src/domain/collectionAdvice.ts';
import { collectionAttempt } from './exports.fixture.ts';

// collectionAttempt() is the shared exports fixture: ACQUISITION_FAILED / PROVIDER_UNAVAILABLE /
// captured_pools 0. Overrides go through parseCollectionAttempt so each fixture stays a valid
// CollectionAttempt, not a hand-rolled literal that could drift from the contract.

test('advice: a catch-up attempt (ACQUISITION_UNAVAILABLE, captured_pools 1 — the smallest non-zero value) reads as catching up, not a provider failure', () => {
  // Arrange: captured_pools 1, not the parser maximum of 8, so the `> 0` threshold at
  // collectionAdvice.ts is actually pinned (a `> 1`/`> 2`/`> 7` mutant would fail this).
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'ACQUISITION_UNAVAILABLE', captured_pools: 1 });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Catching up/);
  assert.match(result, /do not retry/i);
  assert.equal(attemptTone(attempt), 'catching-up');
});

test('advice: a catch-up attempt with the full registry captured (captured_pools 8) is still catching up', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'ACQUISITION_UNAVAILABLE', captured_pools: 8 });
  // Act + Assert
  assert.equal(attemptTone(attempt), 'catching-up');
});

test('advice: the same outcome/reason with captured_pools 0 is a genuine acquisition failure, amber tone', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'ACQUISITION_UNAVAILABLE', captured_pools: 0 });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Inspect provider availability/);
  assert.doesNotMatch(result, /Catching up/);
  assert.equal(attemptTone(attempt), 'amber');
});

test('advice: ACQUISITION_FAILED + RESOURCE_LIMIT with captured_pools > 0 reads as capture volume full (a later pool hit the volume/byte quota after earlier pools already captured)', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'RESOURCE_LIMIT', captured_pools: 1 });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Capture volume full/);
});

test('advice: ACQUISITION_FAILED + RESOURCE_LIMIT with captured_pools 0 reads as an honest union, not a guaranteed automatic prune', () => {
  // Arrange: captured_pools 0 covers the transport request/byte budget, the first pool's own
  // quota check, and managed_ingestion's LogLimitExceeded halt (which faults the session as
  // MANAGED_SOURCE_RECOVERY_FAILED) — the copy cannot promise "next capture" for that last case.
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'RESOURCE_LIMIT', captured_pools: 0 });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Resource limit during acquisition/);
  assert.match(result, /MANAGED_SOURCE_RECOVERY_FAILED/);
  assert.doesNotMatch(result, /Capture volume full/);
  assert.doesNotMatch(result, /frees space automatically/);
});

test('advice: EVALUATION_FAILED + RESOURCE_LIMIT keeps the existing queue/workload wording', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'EVALUATION_FAILED', reason: 'RESOURCE_LIMIT' });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Review the configured pool count and capture bounds/);
  assert.doesNotMatch(result, /Capture volume full/);
  assert.doesNotMatch(result, /Resource limit during acquisition/);
});

test('advice: PROVIDER_UNAVAILABLE keeps its existing provider advice', () => {
  // Arrange
  const attempt = parseCollectionAttempt(collectionAttempt());
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Check the configured provider availability/);
  assert.equal(attemptTone(attempt), 'amber');
});

// ---- attemptLabel: the pill text, including the catch-up override ---------------------
test('attemptLabel: IN_PROGRESS reads "NO TERMINAL OUTCOME"', () => {
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'IN_PROGRESS', reason: null, elapsed_ms: null, finished_at: null });
  assert.equal(attemptLabel(attempt), 'NO TERMINAL OUTCOME');
});
test('attemptLabel: a catch-up attempt reads "CATCHING UP", not the raw outcome', () => {
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'ACQUISITION_UNAVAILABLE', captured_pools: 1 });
  assert.equal(attemptLabel(attempt), 'CATCHING UP');
});
test('attemptLabel: any other outcome reads the raw outcome with underscores replaced by spaces', () => {
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'DECISIONS_RECORDED', reason: null, decision_rows: '1', decision_observation_ids: ['sha256:' + 'a'.repeat(64)] });
  assert.equal(attemptLabel(attempt), 'DECISIONS RECORDED');
});

// ---- attemptTone: pin the amber set and the plain (no-tone) outcomes -------------------
test('attemptTone: EVALUATION_FAILED, DEADLINE_EXCEEDED and IN_PROGRESS are amber; DECISIONS_RECORDED, SUPPRESSED, WORKER_CANCELLED and READINESS_COMPLETED have no tone', () => {
  const evaluationFailed = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'EVALUATION_FAILED', reason: 'TASK_FAILED' });
  const deadlineExceeded = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'DEADLINE_EXCEEDED', reason: 'ACQUISITION_DEADLINE' });
  const inProgress = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'IN_PROGRESS', reason: null, elapsed_ms: null, finished_at: null });
  const decisionsRecorded = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'DECISIONS_RECORDED', reason: null, decision_rows: '1', decision_observation_ids: ['sha256:' + 'b'.repeat(64)] });
  assert.equal(attemptTone(evaluationFailed), 'amber');
  assert.equal(attemptTone(deadlineExceeded), 'amber');
  assert.equal(attemptTone(inProgress), 'amber');
  const suppressed = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'SUPPRESSED', reason: 'GENERATION_FENCED' });
  const workerCancelled = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'WORKER_CANCELLED', reason: 'WORKER_SHUTDOWN' });
  const readinessCompleted = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'READINESS_COMPLETED', reason: null, purpose: 'READINESS' });
  assert.equal(attemptTone(decisionsRecorded), '');
  assert.equal(attemptTone(suppressed), '');
  assert.equal(attemptTone(workerCancelled), '');
  assert.equal(attemptTone(readinessCompleted), '');
});
