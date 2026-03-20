import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import {
    processSingleTarget,
    processMultipleTargets,
    createTargetLoadFailure,
    buildRunPayload,
    summarizeResults,
    buildRequestDetails,
    buildRequestDetailsFromResponse,
    buildExtractDetails,
    buildContentDetails,
    buildChangeDetails,
    buildFailureExtractDetails,
    buildFailureContentDetails,
    buildHashDetails,
    buildFailureHashDetails,
    buildFiles
} from '../lib/core/pipeline.js';
import { createDefaultState, writeState } from '../lib/services/state.js';
import { readTextFile } from '../lib/adapters/filesystem.js';
import { STATUS, ERROR_TYPES, SCHEMA_VERSION } from '../lib/core/constants.js';
import { withTempDir, createFakeHtmlcutBin, withPatchedPath } from './helpers.js';

function createTarget(baseDir, name = 'site') {
    const targetDir = join(baseDir, name);

    return {
        name,
        url: `https://${name}.example.com`,
        request: {
            timeoutMs: 5000,
            maxAttempts: 1,
            retryDelayMs: 1,
            userAgent: 'ffhn/test'
        },
        extract: {
            from: '<main>',
            to: '</main>',
            pattern: 'literal',
            capture: 'inner',
            all: false
        },
        paths: {
            dir: targetDir,
            config: join(targetDir, 'target.toml'),
            state: join(targetDir, 'state.json'),
            current: join(targetDir, 'current.txt'),
            previous: join(targetDir, 'previous.txt')
        }
    };
}

function makeResponse(status, statusText, body) {
    return {
        ok: status >= 200 && status < 300,
        status,
        statusText,
        url: 'https://example.com/final',
        async text() {
            return body;
        }
    };
}

function withMockFetch(mockImpl, fn) {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mockImpl;
    return Promise.resolve()
        .then(fn)
        .finally(() => {
            globalThis.fetch = originalFetch;
        });
}

test('processSingleTarget handles initialized, unchanged, and changed flows', async () => {
    await withTempDir('ffhn-pipeline-', async (baseDir) => {
        const fakeHtmlcut = await createFakeHtmlcutBin(baseDir, 'success');
        const target = createTarget(baseDir, 'demo');
        await mkdir(target.paths.dir, { recursive: true });

        await withPatchedPath(fakeHtmlcut.binDir, async () => {
            await withMockFetch(async () => makeResponse(200, 'OK', '<main>hello</main>'), async () => {
                const initialized = await processSingleTarget(target);
                assert.equal(initialized.status, STATUS.INITIALIZED);
                assert.equal(await readTextFile(target.paths.current), '<MAIN>HELLO</MAIN>');
                assert.equal(initialized.content.current_chars, 18);
                assert.equal(initialized.content.previous_chars, null);
                assert.equal(initialized.content.previous_preview, null);
                assert.equal(initialized.change, null);

                const unchanged = await processSingleTarget(target);
                assert.equal(unchanged.status, STATUS.UNCHANGED);
                assert.equal(unchanged.state.current_hash, initialized.hash.current);
                assert.equal(unchanged.content.delta_chars, 0);
                assert.equal(unchanged.content.previous_preview.text, '<MAIN>HELLO</MAIN>');
                assert.deepEqual(unchanged.change, {
                    mode: 'line',
                    added: {
                        count: 0,
                        items: [],
                        truncated: false
                    },
                    removed: {
                        count: 0,
                        items: [],
                        truncated: false
                    }
                });
            });

            await withMockFetch(async () => makeResponse(200, 'OK', '<main>goodbye</main>'), async () => {
                const changed = await processSingleTarget(target);
                assert.equal(changed.status, STATUS.CHANGED);
                assert.equal(await readTextFile(target.paths.previous), '<MAIN>HELLO</MAIN>');
                assert.equal(changed.state.stats.changes, 1);
                assert.equal(changed.content.delta_chars, 2);
                assert.equal(changed.content.current_preview.text, '<MAIN>GOODBYE</MAIN>');
                assert.equal(changed.content.previous_preview.text, '<MAIN>HELLO</MAIN>');
                assert.deepEqual(changed.change, {
                    mode: 'line',
                    added: {
                        count: 1,
                        items: ['<MAIN>GOODBYE</MAIN>'],
                        truncated: false
                    },
                    removed: {
                        count: 1,
                        items: ['<MAIN>HELLO</MAIN>'],
                        truncated: false
                    }
                });
            });
        });

        const stateOnlyTarget = createTarget(baseDir, 'state-only');
        await mkdir(stateOnlyTarget.paths.dir, { recursive: true });
        const previousState = createDefaultState();
        previousState.last_run_at = '2026-03-13T10:00:00.000Z';
        previousState.last_success_at = '2026-03-13T10:00:00.000Z';
        previousState.current_hash = '0'.repeat(64);
        previousState.stats.runs = 1;
        previousState.stats.successes = 1;
        await writeState(stateOnlyTarget.paths.state, previousState);

        await withPatchedPath(fakeHtmlcut.binDir, async () => {
            await withMockFetch(async () => makeResponse(200, 'OK', '<main>replacement</main>'), async () => {
                const changed = await processSingleTarget(stateOnlyTarget);
                assert.equal(changed.status, STATUS.CHANGED);
                assert.equal(changed.hash.previous, '0'.repeat(64));
                assert.equal(changed.content.previous_chars, null);
                assert.equal(changed.content.delta_chars, null);
                assert.equal(changed.change, null);
                await assert.rejects(readTextFile(stateOnlyTarget.paths.previous), /Failed to read file/);
            });
        });

        const currentOnlyTarget = createTarget(baseDir, 'current-only');
        await mkdir(currentOnlyTarget.paths.dir, { recursive: true });
        await writeFile(currentOnlyTarget.paths.current, '<MAIN>HELLO</MAIN>', 'utf8');

        await withPatchedPath(fakeHtmlcut.binDir, async () => {
            await withMockFetch(async () => makeResponse(200, 'OK', '<main>hello</main>'), async () => {
                const unchanged = await processSingleTarget(currentOnlyTarget);
                assert.equal(unchanged.status, STATUS.UNCHANGED);
                assert.equal(unchanged.hash.previous, unchanged.hash.current);
                assert.equal(unchanged.content.previous_preview.text, '<MAIN>HELLO</MAIN>');
                assert.deepEqual(unchanged.change, {
                    mode: 'line',
                    added: {
                        count: 0,
                        items: [],
                        truncated: false
                    },
                    removed: {
                        count: 0,
                        items: [],
                        truncated: false
                    }
                });
            });
        });
    });
});

test('processSingleTarget categorizes extract, network, filesystem, and state failures', async () => {
    await withTempDir('ffhn-pipeline-', async (baseDir) => {
        const htmlcutFailTarget = createTarget(baseDir, 'htmlcut-fail');
        await mkdir(htmlcutFailTarget.paths.dir, { recursive: true });

        const failBin = await createFakeHtmlcutBin(baseDir, 'fail');
        await withPatchedPath(failBin.binDir, async () => {
            await withMockFetch(async () => makeResponse(200, 'OK', '<main>hello</main>'), async () => {
                const failed = await processSingleTarget(htmlcutFailTarget);
                assert.equal(failed.status, STATUS.FAILED);
                assert.equal(failed.error.type, ERROR_TYPES.EXTRACT);
                assert.equal(failed.request.http_status, 200);
                assert.equal(failed.extract.output_bytes, null);
                assert.equal(failed.content, null);
            });
        });

        const networkTarget = createTarget(baseDir, 'network-fail');
        await mkdir(networkTarget.paths.dir, { recursive: true });
        await withMockFetch(async () => {
            throw new Error('socket fail');
        }, async () => {
            const failed = await processSingleTarget(networkTarget);
            assert.equal(failed.status, STATUS.FAILED);
            assert.equal(failed.error.type, ERROR_TYPES.NETWORK);
            assert.equal(failed.state.stats.failures, 1);
            assert.equal(failed.request.final_url, networkTarget.url);
            assert.equal(failed.request.attempts, 1);
            assert.equal(failed.content, null);
        });

        const fsTarget = createTarget(baseDir, 'fs-fail');
        await mkdir(fsTarget.paths.dir, { recursive: true });
        fsTarget.paths.current = '/dev/null/current.txt';
        const successBin = await createFakeHtmlcutBin(baseDir, 'success');
        await withPatchedPath(successBin.binDir, async () => {
            await withMockFetch(async () => makeResponse(200, 'OK', '<main>hello</main>'), async () => {
                const failed = await processSingleTarget(fsTarget);
                assert.equal(failed.status, STATUS.FAILED);
                assert.equal(failed.error.type, ERROR_TYPES.FILESYSTEM);
                assert.equal(failed.request.http_status, 200);
                assert.equal(failed.extract.output_bytes, 18);
                assert.match(failed.hash.current, /^[a-f0-9]{64}$/);
                assert.equal(failed.content.current_preview.text, '<MAIN>HELLO</MAIN>');
            });
        });

        const stateTarget = createTarget(baseDir, 'state-fail');
        await mkdir(stateTarget.paths.dir, { recursive: true });
        await writeFile(stateTarget.paths.state, '{bad', 'utf8');
        const stateFailed = await processSingleTarget(stateTarget);
        assert.equal(stateFailed.status, STATUS.FAILED);
        assert.equal(stateFailed.error.type, ERROR_TYPES.STATE);
        assert.equal(stateFailed.state, null);

        const unwritableStateTarget = createTarget(baseDir, 'unwritable-state');
        await mkdir(unwritableStateTarget.paths.dir, { recursive: true });
        unwritableStateTarget.paths.state = '/dev/null/state.json';
        await withMockFetch(async () => {
            throw new Error('transport fail');
        }, async () => {
            const failed = await processSingleTarget(unwritableStateTarget);
            assert.equal(failed.status, STATUS.FAILED);
            assert.equal(failed.state.stats.failures, 1);
        });

        const missingStatePathTarget = createTarget(baseDir, 'missing-state-path');
        delete missingStatePathTarget.paths.state;
        await withMockFetch(async () => {
            throw new Error('transport fail');
        }, async () => {
            const failed = await processSingleTarget(missingStatePathTarget);
            assert.equal(failed.status, STATUS.FAILED);
            assert.equal(failed.state.stats.runs, 0);
        });

        const invalidRequestTarget = createTarget(baseDir, 'invalid-request');
        await mkdir(invalidRequestTarget.paths.dir, { recursive: true });
        invalidRequestTarget.request.userAgent = '';
        const invalidRequestFailed = await processSingleTarget(invalidRequestTarget);
        assert.equal(invalidRequestFailed.status, STATUS.FAILED);
        assert.equal(invalidRequestFailed.request, null);

        const sparseErrorTarget = createTarget(baseDir, 'sparse-error');
        await mkdir(sparseErrorTarget.paths.dir, { recursive: true });
        await withMockFetch(async () => {
            throw Object.assign(new Error('sparse'), {
                code: 'SPARSE_ERROR',
                type: ERROR_TYPES.NETWORK,
                details: {}
            });
        }, async () => {
            const failed = await processSingleTarget(sparseErrorTarget);
            assert.equal(failed.status, STATUS.FAILED);
            assert.equal(failed.request, null);
        });

        const malformedDetailsTarget = createTarget(baseDir, 'malformed-details');
        await mkdir(malformedDetailsTarget.paths.dir, { recursive: true });
        await withMockFetch(async () => {
            throw Object.assign(new Error('malformed details'), {
                code: 'MALFORMED_DETAILS',
                type: ERROR_TYPES.NETWORK,
                details: 'nope'
            });
        }, async () => {
            const failed = await processSingleTarget(malformedDetailsTarget);
            assert.equal(failed.status, STATUS.FAILED);
            assert.equal(failed.request, null);
        });
    });
});

test('processMultipleTargets preserves order and validates concurrency', async () => {
    await withTempDir('ffhn-pipeline-', async (baseDir) => {
        assert.deepEqual(await processMultipleTargets([]), []);

        const fakeHtmlcut = await createFakeHtmlcutBin(baseDir, 'success');
        const one = createTarget(baseDir, 'one');
        const two = createTarget(baseDir, 'two');
        await mkdir(one.paths.dir, { recursive: true });
        await mkdir(two.paths.dir, { recursive: true });
        await writeState(two.paths.state, createDefaultState());

        await withPatchedPath(fakeHtmlcut.binDir, async () => {
            await withMockFetch(async (url) => {
                if (url.includes('one')) {
                    return makeResponse(200, 'OK', '<main>one</main>');
                }
                return makeResponse(200, 'OK', '<main>two</main>');
            }, async () => {
                const results = await processMultipleTargets([one, two], 2);
                assert.equal(results.length, 2);
                assert.equal(results[0].name, 'one');
                assert.equal(results[1].name, 'two');
            });
        });

        await assert.rejects(
            processMultipleTargets([one], 0),
            /concurrency must be a positive integer/
        );
    });
});

test('createTargetLoadFailure, summarizeResults, and buildRunPayload produce stable output', () => {
    const loadFailure = createTargetLoadFailure({
        name: 'broken',
        dir: '/tmp/broken',
        configPath: '/tmp/broken/target.toml'
    }, new Error('invalid config'));
    assert.equal(loadFailure.status, STATUS.FAILED);
    assert.equal(loadFailure.error.type, ERROR_TYPES.CONFIG);

    const results = [
        { name: 'init', status: STATUS.INITIALIZED, timing: { total_ms: 5 } },
        { name: 'changed', status: STATUS.CHANGED, timing: { total_ms: 10 } },
        { name: 'same', status: STATUS.UNCHANGED, timing: { total_ms: 20 } },
        { name: 'broken', status: STATUS.FAILED, error: { type: ERROR_TYPES.NETWORK }, timing: { total_ms: 15 } }
    ];
    const summary = summarizeResults(results);
    assert.deepEqual(summary, {
        total_targets: 4,
        initialized: 1,
        changed: 1,
        unchanged: 1,
        failed: 1,
        successful_targets: 3,
        success_rate: 0.75,
        total_target_duration_ms: 50,
        avg_target_duration_ms: 13,
        attention_required: true,
        initialized_target_names: ['init'],
        changed_target_names: ['changed'],
        failed_target_names: ['broken'],
        failure_types: {
            network: 1
        }
    });

    const payload = buildRunPayload(results, 100, 160, '/tmp/watchlist', {
        target: 'changed',
        concurrency: 2
    });
    assert.equal(payload.schema_version, SCHEMA_VERSION);
    assert.equal(payload.duration_ms, 60);
    assert.equal(payload.watchlist, '/tmp/watchlist');
    assert.deepEqual(payload.selection, {
        target: 'changed',
        concurrency: 2
    });
    assert.deepEqual(buildRunPayload([], 100, 100, '/tmp/watchlist').selection, {
        target: null,
        concurrency: 4
    });

    assert.deepEqual(summarizeResults([]), {
        total_targets: 0,
        initialized: 0,
        changed: 0,
        unchanged: 0,
        failed: 0,
        successful_targets: 0,
        success_rate: 0,
        total_target_duration_ms: 0,
        avg_target_duration_ms: 0,
        attention_required: false,
        initialized_target_names: [],
        changed_target_names: [],
        failed_target_names: [],
        failure_types: {}
    });

    assert.deepEqual(summarizeResults([
        { status: STATUS.INITIALIZED, timing: { total_ms: 1 } },
        { status: STATUS.CHANGED, timing: { total_ms: 1 } },
        { status: STATUS.FAILED, error: {}, timing: { total_ms: 1 } }
    ]), {
        total_targets: 3,
        initialized: 1,
        changed: 1,
        unchanged: 0,
        failed: 1,
        successful_targets: 2,
        success_rate: 0.667,
        total_target_duration_ms: 3,
        avg_target_duration_ms: 1,
        attention_required: true,
        initialized_target_names: [],
        changed_target_names: [],
        failed_target_names: [],
        failure_types: {}
    });

    assert.deepEqual(summarizeResults([
        { name: 'one', status: STATUS.FAILED, error: { type: ERROR_TYPES.NETWORK }, timing: { total_ms: 1 } },
        { name: 'two', status: STATUS.FAILED, error: { type: ERROR_TYPES.NETWORK }, timing: { total_ms: 1 } }
    ]), {
        total_targets: 2,
        initialized: 0,
        changed: 0,
        unchanged: 0,
        failed: 2,
        successful_targets: 0,
        success_rate: 0,
        total_target_duration_ms: 2,
        avg_target_duration_ms: 1,
        attention_required: true,
        initialized_target_names: [],
        changed_target_names: [],
        failed_target_names: ['one', 'two'],
        failure_types: {
            network: 2
        }
    });

    assert.deepEqual(summarizeResults([
        { status: STATUS.UNCHANGED }
    ]), {
        total_targets: 1,
        initialized: 0,
        changed: 0,
        unchanged: 1,
        failed: 0,
        successful_targets: 1,
        success_rate: 1,
        total_target_duration_ms: 0,
        avg_target_duration_ms: 0,
        attention_required: false,
        initialized_target_names: [],
        changed_target_names: [],
        failed_target_names: [],
        failure_types: {}
    });
});

test('pipeline helper shapers preserve machine-facing semantics', () => {
    const response = {
        url: 'https://example.com/final',
        status: 200,
        statusText: 'OK',
        attempts: 2,
        contentBytes: 42
    };
    assert.deepEqual(buildRequestDetailsFromResponse(response, 15), {
        final_url: 'https://example.com/final',
        http_status: 200,
        status_text: 'OK',
        attempts: 2,
        body_bytes: 42,
        duration_ms: 15
    });
    assert.deepEqual(buildRequestDetails(response, null, 15), {
        final_url: 'https://example.com/final',
        http_status: 200,
        status_text: 'OK',
        attempts: 2,
        body_bytes: 42,
        duration_ms: 15
    });
    assert.equal(buildRequestDetails(null, new Error('boom'), 5), null);
    assert.equal(buildRequestDetails(null, { details: 'bad-shape' }, 5), null);
    assert.equal(buildRequestDetails(null, { details: {} }, 5), null);
    assert.deepEqual(buildRequestDetails(null, {
        details: {
            status: 503
        }
    }, 9), {
        final_url: null,
        http_status: 503,
        status_text: null,
        attempts: null,
        body_bytes: null,
        duration_ms: 9
    });

    assert.deepEqual(buildExtractDetails('ABC', 7), {
        output_bytes: 3,
        duration_ms: 7
    });
    assert.deepEqual(buildExtractDetails(null, 7), {
        output_bytes: null,
        duration_ms: 7
    });
    assert.deepEqual(buildContentDetails('ABC', 'AB', STATUS.CHANGED), {
        current_chars: 3,
        previous_chars: 2,
        delta_chars: 1,
        current_preview: {
            text: 'ABC',
            truncated: false,
            lines: ['ABC'],
            total_lines: 1,
            lines_truncated: false
        },
        previous_preview: {
            text: 'AB',
            truncated: false,
            lines: ['AB'],
            total_lines: 1,
            lines_truncated: false
        }
    });
    assert.deepEqual(buildContentDetails('ABC', null, STATUS.INITIALIZED), {
        current_chars: 3,
        previous_chars: null,
        delta_chars: null,
        current_preview: {
            text: 'ABC',
            truncated: false,
            lines: ['ABC'],
            total_lines: 1,
            lines_truncated: false
        },
        previous_preview: null
    });
    assert.equal(buildFailureExtractDetails(null, 0), null);
    assert.deepEqual(buildFailureExtractDetails(null, 7), {
        output_bytes: null,
        duration_ms: 7
    });
    assert.equal(buildFailureContentDetails(null, null), null);
    assert.deepEqual(buildFailureContentDetails('ABC', 'AB'), {
        current_chars: 3,
        previous_chars: 2,
        delta_chars: 1,
        current_preview: {
            text: 'ABC',
            truncated: false,
            lines: ['ABC'],
            total_lines: 1,
            lines_truncated: false
        },
        previous_preview: {
            text: 'AB',
            truncated: false,
            lines: ['AB'],
            total_lines: 1,
            lines_truncated: false
        }
    });
    assert.deepEqual(buildFailureContentDetails('ABC', null), {
        current_chars: 3,
        previous_chars: null,
        delta_chars: null,
        current_preview: {
            text: 'ABC',
            truncated: false,
            lines: ['ABC'],
            total_lines: 1,
            lines_truncated: false
        },
        previous_preview: null
    });
    assert.deepEqual(buildFailureContentDetails(null, 'AB'), {
        current_chars: null,
        previous_chars: 2,
        delta_chars: null,
        current_preview: null,
        previous_preview: {
            text: 'AB',
            truncated: false,
            lines: ['AB'],
            total_lines: 1,
            lines_truncated: false
        }
    });
    assert.equal(buildChangeDetails('ABC', null), null);
    assert.deepEqual(buildChangeDetails('ABC', 'ABC'), {
        mode: 'line',
        added: {
            count: 0,
            items: [],
            truncated: false
        },
        removed: {
            count: 0,
            items: [],
            truncated: false
        }
    });
    assert.deepEqual(buildChangeDetails('new\n\nkeep\n', 'old\n\nkeep\n'), {
        mode: 'line',
        added: {
            count: 1,
            items: ['new'],
            truncated: false
        },
        removed: {
            count: 1,
            items: ['old'],
            truncated: false
        }
    });
    assert.deepEqual(buildChangeDetails('repeat\nrepeat\nkeep\n', 'repeat\nkeep\n'), {
        mode: 'line',
        added: {
            count: 1,
            items: ['repeat'],
            truncated: false
        },
        removed: {
            count: 0,
            items: [],
            truncated: false
        }
    });
    assert.deepEqual(
        buildChangeDetails(
            'one\ntwo\nthree\nfour\nfive\nsix\nkeep\n',
            'keep\n'
        ),
        {
            mode: 'line',
            added: {
                count: 6,
                items: ['one', 'two', 'three', 'four', 'five'],
                truncated: true
            },
            removed: {
                count: 0,
                items: [],
                truncated: false
            }
        }
    );
    assert.deepEqual(buildFailureExtractDetails('ABC', 0), {
        output_bytes: 3,
        duration_ms: 0
    });
    const longContent = 'x'.repeat(200);
    assert.deepEqual(buildContentDetails(longContent, null, STATUS.INITIALIZED).current_preview, {
        text: 'x'.repeat(160),
        truncated: true,
        lines: ['x'.repeat(200)],
        total_lines: 1,
        lines_truncated: false
    });
    assert.deepEqual(buildContentDetails('one\n\ntwo\n\nthree\n\nfour\n\nfive\n\nsix', null, STATUS.INITIALIZED).current_preview, {
        text: 'one\n\ntwo\n\nthree\n\nfour\n\nfive\n\nsix',
        truncated: false,
        lines: ['one', 'two', 'three', 'four', 'five'],
        total_lines: 6,
        lines_truncated: true
    });

    assert.deepEqual(buildHashDetails('a', 'b'), {
        current: 'a',
        previous: 'b',
        algorithm: 'sha256'
    });
    assert.equal(buildFailureHashDetails(null, null), null);
    assert.deepEqual(buildFailureHashDetails('a', null), {
        current: 'a',
        previous: null,
        algorithm: 'sha256'
    });

    assert.deepEqual(buildFiles({
        dir: '/tmp/demo',
        config: '/tmp/demo/target.toml',
        state: '/tmp/demo/state.json',
        current: '/tmp/demo/current.txt',
        previous: '/tmp/demo/previous.txt'
    }), {
        directory: '/tmp/demo',
        config: '/tmp/demo/target.toml',
        state: '/tmp/demo/state.json',
        current: '/tmp/demo/current.txt',
        previous: '/tmp/demo/previous.txt'
    });
});
