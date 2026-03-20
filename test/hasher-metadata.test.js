import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { computeHash, compareHashes } from '../lib/services/hasher.js';
import {
    createDefaultState,
    readState,
    writeState,
    recordSuccess,
    recordFailure,
    validateState
} from '../lib/services/state.js';
import { SCHEMA_VERSION, STATUS } from '../lib/core/constants.js';
import { withTempDir } from './helpers.js';

function createSuccessfulState() {
    const state = createDefaultState();
    state.last_run_at = '2026-03-13T10:00:00.000Z';
    state.last_success_at = '2026-03-13T10:00:00.000Z';
    state.current_hash = 'hash-current';
    state.stats.runs = 1;
    state.stats.successes = 1;
    return state;
}

function createChangedState() {
    const state = createSuccessfulState();
    state.last_change_at = '2026-03-13T10:01:00.000Z';
    state.previous_hash = 'hash-previous';
    state.stats.changes = 1;
    return state;
}

test('computeHash accepts empty strings and compareHashes detects changes', () => {
    const emptyHash = computeHash('');
    assert.equal(typeof emptyHash, 'string');
    assert.equal(compareHashes(emptyHash, emptyHash), false);
    assert.equal(compareHashes(computeHash('a'), computeHash('b')), true);
    assert.throws(() => computeHash(null), /Content must be a string/);
});

test('state read/write and mutation helpers are strict and durable', async () => {
    await withTempDir('ffhn-state-', async (baseDir) => {
        const statePath = join(baseDir, 'state.json');

        const state = await readState(statePath);
        assert.equal(state.schema_version, SCHEMA_VERSION);
        assert.equal(state.stats.runs, 0);

        recordSuccess(state, 'hash-1', STATUS.INITIALIZED, '2026-03-13T10:00:00.000Z');
        assert.equal(state.current_hash, 'hash-1');
        assert.equal(state.previous_hash, null);

        recordSuccess(state, 'hash-2', STATUS.CHANGED, '2026-03-13T10:01:00.000Z');
        assert.equal(state.previous_hash, 'hash-1');
        assert.equal(state.stats.changes, 1);

        recordSuccess(state, 'hash-2', STATUS.UNCHANGED, '2026-03-13T10:02:00.000Z');
        assert.equal(state.stats.consecutive_changes, 0);

        recordFailure(state, new Error('boom'), '2026-03-13T10:03:00.000Z');
        assert.equal(state.stats.failures, 1);
        assert.equal(state.last_error.message, 'boom');

        await writeState(statePath, state);
        const reloadedState = await readState(statePath);
        assert.deepEqual(reloadedState, state);
    });
});

test('readState and validateState reject corrupt files and invalid shapes', async () => {
    await withTempDir('ffhn-state-', async (baseDir) => {
        const invalidJsonPath = join(baseDir, 'bad.json');
        await writeFile(invalidJsonPath, '{oops', 'utf8');
        await assert.rejects(readState(invalidJsonPath), /not valid JSON/);

        await assert.rejects(readState(baseDir), /Failed to read state file/);

        const wrongSchemaPath = join(baseDir, 'wrong.json');
        await writeFile(wrongSchemaPath, JSON.stringify({
            ...createDefaultState(),
            schema_version: SCHEMA_VERSION + 1
        }), 'utf8');
        await assert.rejects(readState(wrongSchemaPath), new RegExp(`schema_version must be ${SCHEMA_VERSION}`));

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: -1
                }
            }),
            /stats.runs must be a non-negative integer/
        );

        assert.throws(
            () => validateState(null),
            /State must be an object/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: []
            }),
            /State stats must be an object/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(''),
                created_at: ''
            }),
            /State created_at must be a non-empty string/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                last_error: []
            }),
            /State last_error must be null or an object/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                last_error: {
                    code: 'X',
                    type: 'state',
                    message: '',
                    at: 'now'
                }
            }),
            /State last_error.message must be a non-empty string/
        );

        assert.throws(
            () => recordSuccess(createDefaultState(), 'hash', 'broken'),
            /Unsupported success status/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                current_hash: 'hash'
            }),
            /current_hash must be null when stats.successes is 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: 1,
                    successes: 2,
                    failures: 0,
                    changes: 0,
                    consecutive_failures: 0,
                    consecutive_changes: 0
                }
            }),
            /stats.successes must not exceed stats.runs/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                last_success_at: '2026-03-13T10:00:00.000Z'
            }),
            /last_success_at must be null when stats.successes is 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                last_run_at: '2026-03-13T10:00:00.000Z'
            }),
            /last_run_at must be null when stats.runs is 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: 1,
                    successes: 1,
                    failures: 0,
                    changes: 0,
                    consecutive_failures: 0,
                    consecutive_changes: 0
                },
                current_hash: 'hash',
                last_success_at: null
            }),
            /last_success_at must be set when stats.successes is greater than 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: 1,
                    successes: 0,
                    failures: 2,
                    changes: 0,
                    consecutive_failures: 0,
                    consecutive_changes: 0
                }
            }),
            /stats.failures must not exceed stats.runs/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                stats: {
                    runs: 1,
                    successes: 1,
                    failures: 0,
                    changes: 2,
                    consecutive_failures: 0,
                    consecutive_changes: 0
                }
            }),
            /stats.changes must not exceed stats.successes/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: 1,
                    successes: 0,
                    failures: 1,
                    changes: 0,
                    consecutive_failures: 2,
                    consecutive_changes: 0
                }
            }),
            /stats.consecutive_failures must not exceed stats.failures/
        );

        assert.throws(
            () => validateState({
                ...createChangedState(),
                stats: {
                    runs: 1,
                    successes: 1,
                    failures: 0,
                    changes: 1,
                    consecutive_failures: 0,
                    consecutive_changes: 2
                }
            }),
            /stats.consecutive_changes must not exceed stats.changes/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                last_change_at: '2026-03-13T10:01:00.000Z'
            }),
            /last_change_at must be null when stats.changes is 0/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                stats: {
                    runs: 1,
                    successes: 1,
                    failures: 0,
                    changes: 1,
                    consecutive_failures: 0,
                    consecutive_changes: 0
                }
            }),
            /last_change_at must be set when stats.changes is greater than 0/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                current_hash: null
            }),
            /current_hash must be set when stats.successes is greater than 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                previous_hash: 'hash-previous'
            }),
            /previous_hash must be null when current_hash is null/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                previous_hash: 'hash-previous'
            }),
            /previous_hash must be null when stats.changes is 0/
        );

        assert.throws(
            () => validateState({
                ...createDefaultState(),
                stats: {
                    runs: 1,
                    successes: 0,
                    failures: 1,
                    changes: 0,
                    consecutive_failures: 1,
                    consecutive_changes: 0
                }
            }),
            /last_run_at must be set when stats.runs is greater than 0/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                last_error: {
                    code: 'X',
                    type: 'network',
                    message: 'boom',
                    at: '2026-03-13T10:05:00.000Z'
                }
            }),
            /last_error.at must match last_run_at/
        );

        assert.throws(
            () => validateState({
                ...createSuccessfulState(),
                last_run_at: '2026-03-13T10:05:00.000Z'
            }),
            /last_success_at must match last_run_at when last_error is null/
        );
    });
});
