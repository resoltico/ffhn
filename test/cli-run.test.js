import test from 'node:test';
import assert from 'node:assert/strict';

import { runCommand, statusCommand } from '../lib/cli.js';
import { EXIT_CODES } from '../lib/core/constants.js';
import { configError } from '../lib/utils/error.js';
import { computeHash } from '../lib/services/hasher.js';
import { createDefaultState } from '../lib/services/state.js';

function makeAsyncTargetIterator(targets) {
    return async function* discoverTargetsFn() {
        for (const target of targets) {
            yield target;
        }
    };
}

function createTrackedState(currentContent, previousContent = null) {
    const state = createDefaultState('2026-03-13T10:00:00.000Z');
    state.last_run_at = '2026-03-13T10:03:00.000Z';
    state.last_success_at = '2026-03-13T10:03:00.000Z';
    state.current_hash = computeHash(currentContent);
    state.stats.runs = 1;
    state.stats.successes = 1;

    if (previousContent !== null) {
        state.previous_hash = computeHash(previousContent);
        state.last_change_at = '2026-03-13T10:02:00.000Z';
        state.stats.changes = 1;
    }

    return state;
}

function createFailedState(currentContent, previousContent = null) {
    const state = createTrackedState(currentContent, previousContent);
    state.last_run_at = '2026-03-13T10:04:00.000Z';
    state.last_error = {
        code: 'REQUEST_FAILED',
        type: 'network',
        message: 'boom',
        at: '2026-03-13T10:04:00.000Z'
    };
    state.stats.runs = 2;
    state.stats.failures = 1;
    state.stats.consecutive_failures = 1;
    state.stats.consecutive_changes = 0;
    return state;
}

function createLastChangedState(currentContent, previousContent) {
    const state = createTrackedState(currentContent, previousContent);
    state.last_change_at = state.last_run_at;
    return state;
}

test('runCommand preserves discovery order and mixes loaded and failed targets', async () => {
    const discoveredTargets = [
        { name: 'a', dir: '/tmp/a', configPath: '/tmp/a/target.toml' },
        { name: 'b', dir: '/tmp/b', configPath: '/tmp/b/target.toml' },
        { name: 'c', dir: '/tmp/c', configPath: '/tmp/c/target.toml' }
    ];
    const loadCalls = [];
    const processCalls = [];

    const result = await runCommand({
        watchlist: '/tmp/watchlist',
        concurrency: 2
    }, {
        nowFn: (() => {
            let now = 0;
            return () => {
                now += 10;
                return now;
            };
        })(),
        discoverTargetsFn: makeAsyncTargetIterator(discoveredTargets),
        async loadTargetFn(discoveredTarget) {
            loadCalls.push(discoveredTarget.name);
            if (discoveredTarget.name === 'b') {
                throw configError('TARGET_CONFIG_INVALID', 'bad target');
            }
            return {
                name: discoveredTarget.name,
                url: `https://${discoveredTarget.name}.example.com`,
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async processMultipleTargetsFn(targets, concurrency) {
            processCalls.push({ targets: targets.map((target) => target.name), concurrency });
            return [
                { name: 'a', status: 'initialized', timing: { total_ms: 15 } },
                { name: 'c', status: 'unchanged', timing: { total_ms: 5 } }
            ];
        }
    });

    assert.deepEqual(loadCalls, ['a', 'b', 'c']);
    assert.deepEqual(processCalls, [{ targets: ['a', 'c'], concurrency: 2 }]);
    assert.equal(result.command, 'run');
    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.deepEqual(result.selection, {
        target: null,
        concurrency: 2
    });
    assert.equal(result.targets[0].name, 'a');
    assert.equal(result.targets[1].name, 'b');
    assert.equal(result.targets[1].status, 'failed');
    assert.equal(result.targets[2].name, 'c');
    assert.equal(result.duration_ms, 10);
    assert.deepEqual(result.summary, {
        total_targets: 3,
        initialized: 1,
        changed: 0,
        unchanged: 1,
        failed: 1,
        successful_targets: 2,
        success_rate: 0.667,
        total_target_duration_ms: 20,
        avg_target_duration_ms: 7,
        attention_required: true,
        initialized_target_names: ['a'],
        changed_target_names: [],
        failed_target_names: ['b'],
        failure_types: {
            config: 1
        }
    });
});

test('runCommand throws config error when no targets exist', async () => {
    await assert.rejects(
        runCommand({ watchlist: '/tmp/watchlist' }, {
            nowFn: Date.now,
            discoverTargetsFn: makeAsyncTargetIterator([]),
            async loadTargetFn() {},
            async processMultipleTargetsFn() {
                return [];
            }
        }),
        /No targets found/
    );
});

test('runCommand throws config error when target selector misses', async () => {
    await assert.rejects(
        runCommand({ watchlist: '/tmp/watchlist', target: 'missing' }, {
            nowFn: Date.now,
            discoverTargetsFn: makeAsyncTargetIterator([
                { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
            ]),
            async loadTargetFn() {},
            async processMultipleTargetsFn() {
                return [];
            }
        }),
        /Target not found: missing/
    );
});

test('runCommand and statusCommand reject malformed target selectors before discovery', async () => {
    await assert.rejects(
        runCommand({ watchlist: '/tmp/watchlist', target: 'bad/name' }, {
            nowFn: Date.now,
            discoverTargetsFn: makeAsyncTargetIterator([]),
            async loadTargetFn() {
                throw new Error('should not be called');
            },
            async processMultipleTargetsFn() {
                return [];
            }
        }),
        /Invalid --target value/
    );

    await assert.rejects(
        statusCommand({ watchlist: '/tmp/watchlist', target: 'bad/name' }, {
            discoverTargetsFn: makeAsyncTargetIterator([]),
            async loadTargetFn() {
                throw new Error('should not be called');
            },
            async pathExistsFn() {
                return false;
            },
            async readStateFn() {
                throw new Error('should not be called');
            }
        }),
        /Invalid --target value/
    );
});

test('statusCommand marks invalid targets and uses config exit code', async () => {
    const discoveredTargets = [
        { name: 'good', dir: '/tmp/good', configPath: '/tmp/good/target.toml' },
        { name: 'bad', dir: '/tmp/bad', configPath: '/tmp/bad/target.toml' }
    ];

    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator(discoveredTargets),
        async loadTargetFn(discoveredTarget) {
            if (discoveredTarget.name === 'bad') {
                throw configError('TARGET_CONFIG_INVALID', 'broken');
            }

            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn(statePath) {
            return statePath.includes('/good/');
        },
        async readStateFn() {
            return createTrackedState('current text');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'current text';
            }
            return null;
        }
    });

    assert.equal(result.command, 'status');
    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.deepEqual(result.selection, {
        target: null
    });
    assert.equal(result.count, 2);
    assert.equal(result.invalid_targets, 1);
    assert.equal(result.targets[0].status, 'ready');
    assert.deepEqual(result.targets[0].last_run, {
        status: 'initialized',
        at: '2026-03-13T10:03:00.000Z',
        success_at: '2026-03-13T10:03:00.000Z',
        change_at: null,
        error: null
    });
    assert.equal(result.targets[1].status, 'invalid');
    assert.deepEqual(result.summary, {
        total_targets: 2,
        ready: 1,
        pending: 0,
        invalid: 1,
        attention_required: true,
        ready_target_names: ['good'],
        pending_target_names: [],
        invalid_target_names: ['bad'],
        invalid_codes: {
            TARGET_CONFIG_INVALID: 1
        },
        last_initialized: 1,
        last_changed: 0,
        last_unchanged: 0,
        last_failed: 0,
        never_run: 0,
        last_initialized_target_names: ['good'],
        last_changed_target_names: [],
        last_failed_target_names: [],
        never_run_target_names: []
    });
});

test('statusCommand reports unreadable state files as invalid targets', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            throw new Error('broken state');
        },
        async readTextFileIfExistsFn() {
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.equal(result.targets[0].status, 'invalid');
    assert.equal(result.targets[0].error.message, 'broken state');
});

test('statusCommand reports ready targets without attention', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createTrackedState('current text', 'old');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'current text';
            }
            if (filePath.endsWith('/previous.txt')) {
                return 'old';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.SUCCESS);
    assert.equal(result.summary.ready, 1);
    assert.equal(result.summary.attention_required, false);
    assert.deepEqual(result.summary.ready_target_names, ['demo']);
    assert.deepEqual(result.targets[0].last_run, {
        status: 'unchanged',
        at: '2026-03-13T10:03:00.000Z',
        success_at: '2026-03-13T10:03:00.000Z',
        change_at: '2026-03-13T10:02:00.000Z',
        error: null
    });
    assert.deepEqual(result.targets[0].content, {
        current_chars: 12,
        previous_chars: 3,
        delta_chars: 9,
        current_preview: {
            text: 'current text',
            truncated: false,
            lines: ['current text'],
            total_lines: 1,
            lines_truncated: false
        },
        previous_preview: {
            text: 'old',
            truncated: false,
            lines: ['old'],
            total_lines: 1,
            lines_truncated: false
        }
    });
    assert.deepEqual(result.targets[0].change, {
        mode: 'line',
        added: {
            count: 1,
            items: ['current text'],
            truncated: false
        },
        removed: {
            count: 1,
            items: ['old'],
            truncated: false
        }
    });
    assert.deepEqual(result.targets[0].artifacts, {
        consistent: true,
        issues: [],
        current: {
            exists: true,
            chars: 12,
            hash: computeHash('current text'),
            state_hash: computeHash('current text'),
            matches_state_hash: true
        },
        previous: {
            exists: true,
            chars: 3,
            hash: computeHash('old'),
            state_hash: computeHash('old'),
            matches_state_hash: true
        }
    });
});

test('statusCommand reports pending targets when no state file exists yet', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return false;
        },
        async readStateFn() {
            throw new Error('should not be called');
        },
        async readTextFileIfExistsFn() {
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.SUCCESS);
    assert.equal(result.targets[0].status, 'pending');
    assert.equal(result.targets[0].state, null);
    assert.equal(result.targets[0].change, null);
    assert.deepEqual(result.targets[0].last_run, {
        status: 'never',
        at: null,
        success_at: null,
        change_at: null,
        error: null
    });
    assert.equal(result.summary.pending, 1);
    assert.equal(result.summary.attention_required, true);
    assert.deepEqual(result.targets[0].artifacts, {
        consistent: true,
        issues: [],
        current: {
            exists: false,
            chars: null,
            hash: null,
            state_hash: null,
            matches_state_hash: null
        },
        previous: {
            exists: false,
            chars: null,
            hash: null,
            state_hash: null,
            matches_state_hash: null
        }
    });
});

test('statusCommand reports never-run state files without treating them as pending', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createDefaultState('2026-03-13T10:00:00.000Z');
        },
        async readTextFileIfExistsFn() {
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.SUCCESS);
    assert.equal(result.targets[0].status, 'ready');
    assert.deepEqual(result.targets[0].last_run, {
        status: 'never',
        at: null,
        success_at: null,
        change_at: null,
        error: null
    });
    assert.equal(result.summary.attention_required, false);
    assert.equal(result.summary.never_run, 1);
    assert.deepEqual(result.summary.never_run_target_names, ['demo']);
});

test('statusCommand honors target selector and errors when it misses', async () => {
    const discoveredTargets = [
        { name: 'alpha', dir: '/tmp/alpha', configPath: '/tmp/alpha/target.toml' },
        { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
    ];

    const filtered = await statusCommand({ watchlist: '/tmp/watchlist', target: 'demo' }, {
        discoverTargetsFn: makeAsyncTargetIterator(discoveredTargets),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return false;
        },
        async readStateFn() {
            throw new Error('should not be called');
        },
        async readTextFileIfExistsFn() {
            return null;
        }
    });

    assert.equal(filtered.count, 1);
    assert.equal(filtered.targets[0].name, 'demo');
    assert.deepEqual(filtered.selection, {
        target: 'demo'
    });
    assert.equal(filtered.targets[0].content, null);
    assert.equal(filtered.targets[0].change, null);
    assert.deepEqual(filtered.targets[0].last_run, {
        status: 'never',
        at: null,
        success_at: null,
        change_at: null,
        error: null
    });

    await assert.rejects(
        statusCommand({ watchlist: '/tmp/watchlist', target: 'missing' }, {
            discoverTargetsFn: makeAsyncTargetIterator(discoveredTargets),
            async loadTargetFn() {
                throw new Error('should not be called');
            },
            async pathExistsFn() {
                return false;
            },
            async readStateFn() {
                throw new Error('should not be called');
            },
            async readTextFileIfExistsFn() {
                return null;
            }
        }),
        /Target not found: missing/
    );
});

test('statusCommand marks orphaned stored content without state as invalid', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return false;
        },
        async readStateFn() {
            throw new Error('should not be called');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'pending preview';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.equal(result.targets[0].status, 'invalid');
    assert.equal(result.targets[0].error.code, 'STATUS_ARTIFACTS_INVALID');
    assert.deepEqual(result.targets[0].artifacts.issues, [
        {
            code: 'ORPHANED_CURRENT_FILE',
            message: 'current.txt exists but state.json is missing'
        }
    ]);
    assert.deepEqual(result.summary.invalid_codes, {
        ORPHANED_CURRENT_FILE: 1
    });
});

test('statusCommand marks hash drift between state and stored files as invalid', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createTrackedState('expected text');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'tampered text';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.equal(result.targets[0].status, 'invalid');
    assert.equal(result.targets[0].error.code, 'STATUS_ARTIFACTS_INVALID');
    assert.deepEqual(result.targets[0].artifacts.issues, [
        {
            code: 'CURRENT_HASH_MISMATCH',
            message: 'current.txt content does not match state.current_hash'
        }
    ]);
    assert.deepEqual(result.summary.invalid_codes, {
        CURRENT_HASH_MISMATCH: 1
    });
});

test('statusCommand surfaces all stored-artifact mismatch codes', async () => {
    const discoveredTargets = [
        { name: 'orphaned-previous', dir: '/tmp/orphaned-previous', configPath: '/tmp/orphaned-previous/target.toml' },
        { name: 'current-unexpected', dir: '/tmp/current-unexpected', configPath: '/tmp/current-unexpected/target.toml' },
        { name: 'current-missing', dir: '/tmp/current-missing', configPath: '/tmp/current-missing/target.toml' },
        { name: 'previous-unexpected', dir: '/tmp/previous-unexpected', configPath: '/tmp/previous-unexpected/target.toml' },
        { name: 'previous-missing', dir: '/tmp/previous-missing', configPath: '/tmp/previous-missing/target.toml' },
        { name: 'previous-mismatch', dir: '/tmp/previous-mismatch', configPath: '/tmp/previous-mismatch/target.toml' }
    ];

    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator(discoveredTargets),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn(statePath) {
            return !statePath.includes('/orphaned-previous/');
        },
        async readStateFn(statePath) {
            if (statePath.includes('/current-unexpected/')) {
                return createDefaultState();
            }
            if (statePath.includes('/current-missing/')) {
                return createTrackedState('expected current');
            }
            if (statePath.includes('/previous-unexpected/')) {
                return createTrackedState('stable current');
            }
            if (statePath.includes('/previous-missing/')) {
                return createTrackedState('stable current', 'expected previous');
            }
            if (statePath.includes('/previous-mismatch/')) {
                return createTrackedState('stable current', 'expected previous');
            }
            throw new Error(`Unexpected state path: ${statePath}`);
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/orphaned-previous/previous.txt')) {
                return 'orphan previous';
            }
            if (filePath.endsWith('/current-unexpected/current.txt')) {
                return 'unexpected current';
            }
            if (filePath.endsWith('/previous-unexpected/current.txt')) {
                return 'stable current';
            }
            if (filePath.endsWith('/previous-unexpected/previous.txt')) {
                return 'unexpected previous';
            }
            if (filePath.endsWith('/previous-missing/current.txt')) {
                return 'stable current';
            }
            if (filePath.endsWith('/previous-mismatch/current.txt')) {
                return 'stable current';
            }
            if (filePath.endsWith('/previous-mismatch/previous.txt')) {
                return 'tampered previous';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.deepEqual(result.summary.invalid_codes, {
        ORPHANED_PREVIOUS_FILE: 1,
        CURRENT_FILE_UNEXPECTED: 1,
        CURRENT_FILE_MISSING: 1,
        PREVIOUS_FILE_UNEXPECTED: 1,
        PREVIOUS_FILE_MISSING: 1,
        PREVIOUS_HASH_MISMATCH: 1
    });
    assert.deepEqual(result.targets.map((target) => target.artifacts.issues[0].code), [
        'ORPHANED_PREVIOUS_FILE',
        'CURRENT_FILE_UNEXPECTED',
        'CURRENT_FILE_MISSING',
        'PREVIOUS_FILE_UNEXPECTED',
        'PREVIOUS_FILE_MISSING',
        'PREVIOUS_HASH_MISMATCH'
    ]);
});

test('statusCommand marks unreadable stored content as invalid', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createTrackedState('current text');
        },
        async readTextFileIfExistsFn() {
            throw new Error('preview broken');
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.CONFIG_ERROR);
    assert.equal(result.targets[0].status, 'invalid');
    assert.equal(result.targets[0].error.message, 'preview broken');
});

test('statusCommand surfaces last failed runs and marks attention required', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createFailedState('current text', 'old');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'current text';
            }
            if (filePath.endsWith('/previous.txt')) {
                return 'old';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.SUCCESS);
    assert.equal(result.targets[0].status, 'ready');
    assert.deepEqual(result.targets[0].last_run, {
        status: 'failed',
        at: '2026-03-13T10:04:00.000Z',
        success_at: '2026-03-13T10:03:00.000Z',
        change_at: '2026-03-13T10:02:00.000Z',
        error: {
            code: 'REQUEST_FAILED',
            type: 'network',
            message: 'boom',
            at: '2026-03-13T10:04:00.000Z'
        }
    });
    assert.equal(result.summary.attention_required, true);
    assert.equal(result.summary.last_failed, 1);
    assert.deepEqual(result.summary.last_failed_target_names, ['demo']);
});

test('statusCommand surfaces last changed runs and marks attention required', async () => {
    const result = await statusCommand({ watchlist: '/tmp/watchlist' }, {
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async pathExistsFn() {
            return true;
        },
        async readStateFn() {
            return createLastChangedState('current text', 'old');
        },
        async readTextFileIfExistsFn(filePath) {
            if (filePath.endsWith('/current.txt')) {
                return 'current text';
            }
            if (filePath.endsWith('/previous.txt')) {
                return 'old';
            }
            return null;
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.SUCCESS);
    assert.equal(result.targets[0].status, 'ready');
    assert.deepEqual(result.targets[0].last_run, {
        status: 'changed',
        at: '2026-03-13T10:03:00.000Z',
        success_at: '2026-03-13T10:03:00.000Z',
        change_at: '2026-03-13T10:03:00.000Z',
        error: null
    });
    assert.equal(result.summary.attention_required, true);
    assert.equal(result.summary.last_changed, 1);
    assert.deepEqual(result.summary.last_changed_target_names, ['demo']);
});

test('runCommand prioritizes dependency failures over runtime target failures', async () => {
    const result = await runCommand({
        watchlist: '/tmp/watchlist'
    }, {
        nowFn: Date.now,
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async processMultipleTargetsFn() {
            return [
                {
                    name: 'demo',
                    status: 'failed',
                    error: {
                        type: 'dependency'
                    },
                    timing: { total_ms: 1 }
                }
            ];
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.DEPENDENCY_ERROR);
});

test('runCommand returns target failure for ordinary target errors', async () => {
    const result = await runCommand({
        watchlist: '/tmp/watchlist'
    }, {
        nowFn: Date.now,
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async processMultipleTargetsFn() {
            return [
                {
                    name: 'demo',
                    status: 'failed',
                    error: {
                        type: 'network'
                    },
                    timing: { total_ms: 1 }
                }
            ];
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.TARGET_FAILURE);
});

test('runCommand prioritizes internal failures above all other failures', async () => {
    const result = await runCommand({
        watchlist: '/tmp/watchlist'
    }, {
        nowFn: Date.now,
        discoverTargetsFn: makeAsyncTargetIterator([
            { name: 'demo', dir: '/tmp/demo', configPath: '/tmp/demo/target.toml' }
        ]),
        async loadTargetFn(discoveredTarget) {
            return {
                name: discoveredTarget.name,
                url: 'https://example.com',
                paths: {
                    dir: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/state.json`,
                    current: `${discoveredTarget.dir}/current.txt`,
                    previous: `${discoveredTarget.dir}/previous.txt`
                }
            };
        },
        async processMultipleTargetsFn() {
            return [
                {
                    name: 'demo',
                    status: 'failed',
                    error: {
                        type: 'internal'
                    },
                    timing: { total_ms: 1 }
                }
            ];
        }
    });

    assert.equal(result.exit_code, EXIT_CODES.INTERNAL_ERROR);
});
