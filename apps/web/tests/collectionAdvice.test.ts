import assert from 'node:assert/strict';
import test from 'node:test';
import { parseCollectionAttempt } from '../src/api/collection.ts';
import { advice, attemptTone } from '../src/domain/collectionAdvice.ts';
import { collectionAttempt } from './exports.fixture.ts';

// collectionAttempt() is the shared exports fixture: ACQUISITION_FAILED / PROVIDER_UNAVAILABLE /
// captured_pools 0. Overrides go through parseCollectionAttempt so each fixture stays a valid
// CollectionAttempt, not a hand-rolled literal that could drift from the contract.

test('advice: a catch-up attempt (ACQUISITION_UNAVAILABLE, captured_pools > 0) reads as catching up, not a provider failure', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'ACQUISITION_UNAVAILABLE', captured_pools: 8 });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Catching up/);
  assert.match(result, /do not retry/i);
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

test('advice: ACQUISITION_FAILED + RESOURCE_LIMIT reads as capture volume full, not the workload wording', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), reason: 'RESOURCE_LIMIT' });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Capture volume full/);
});

test('advice: EVALUATION_FAILED + RESOURCE_LIMIT keeps the existing queue/workload wording', () => {
  // Arrange
  const attempt = parseCollectionAttempt({ ...collectionAttempt(), outcome: 'EVALUATION_FAILED', reason: 'RESOURCE_LIMIT' });
  // Act
  const result = advice(attempt);
  // Assert
  assert.match(result, /Review the configured pool count and capture bounds/);
  assert.doesNotMatch(result, /Capture volume full/);
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
