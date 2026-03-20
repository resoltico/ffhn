import test from 'node:test';
import assert from 'node:assert/strict';

import {
    FfhnError,
    normalizeError,
    serializeError,
    toFfhnError,
    usageError,
    configError,
    dependencyError,
    targetFailure
} from '../lib/utils/error.js';
import { ERROR_TYPES, EXIT_CODES } from '../lib/core/constants.js';

test('normalizeError preserves existing errors and stringifies primitives', () => {
    const error = new Error('boom');
    assert.equal(normalizeError(error), error);
    assert.equal(normalizeError('plain').message, 'plain');
    assert.equal(normalizeError(123).message, '123');
});

test('normalizeError preserves well-known object fields', () => {
    const error = normalizeError({
        message: 'object failure',
        name: 'CustomError',
        status: 502,
        code: 'ECODE',
        type: 'network',
        exitCode: 4,
        details: { key: 'value' }
    });

    assert.equal(error.name, 'CustomError');
    assert.equal(error.status, 502);
    assert.equal(error.code, 'ECODE');
    assert.equal(error.type, 'network');
    assert.equal(error.exitCode, 4);
    assert.deepEqual(error.details, { key: 'value' });

    const stringified = normalizeError({ message: '' });
    assert.equal(stringified.message, '[object Object]');
});

test('FfhnError helpers attach type and exit metadata', () => {
    const usage = usageError('BAD_USAGE', 'bad usage');
    const config = configError('BAD_CONFIG', 'bad config');
    const dependency = dependencyError('MISSING_TOOL', 'missing tool');
    const failure = targetFailure('FETCH_FAILED', ERROR_TYPES.NETWORK, 'fetch failed');

    assert.equal(usage.code, 'BAD_USAGE');
    assert.equal(usage.type, ERROR_TYPES.USAGE);
    assert.equal(usage.exitCode, EXIT_CODES.USAGE_ERROR);

    assert.equal(config.type, ERROR_TYPES.CONFIG);
    assert.equal(config.exitCode, EXIT_CODES.CONFIG_ERROR);

    assert.equal(dependency.type, ERROR_TYPES.DEPENDENCY);
    assert.equal(dependency.exitCode, EXIT_CODES.DEPENDENCY_ERROR);

    assert.equal(failure.type, ERROR_TYPES.NETWORK);
    assert.equal(failure.exitCode, EXIT_CODES.TARGET_FAILURE);
});

test('toFfhnError wraps non-ffhn errors and serializeError emits stable payloads', () => {
    const wrapped = toFfhnError(new Error('boom'), {
        code: 'WRAPPED',
        type: ERROR_TYPES.INTERNAL,
        exitCode: EXIT_CODES.INTERNAL_ERROR
    });

    assert.ok(wrapped instanceof FfhnError);
    assert.equal(wrapped.code, 'WRAPPED');

    const payload = serializeError(wrapped);
    assert.deepEqual(payload, {
        code: 'WRAPPED',
        type: ERROR_TYPES.INTERNAL,
        message: 'boom',
        details: null
    });

    const existing = new FfhnError('EXISTING', 'keep me');
    assert.equal(toFfhnError(existing, {
        code: 'IGNORED',
        type: ERROR_TYPES.INTERNAL,
        exitCode: EXIT_CODES.INTERNAL_ERROR
    }), existing);

    assert.deepEqual(serializeError(new Error('raw')), {
        code: 'INTERNAL_ERROR',
        type: ERROR_TYPES.INTERNAL,
        message: 'raw',
        details: null
    });
});
