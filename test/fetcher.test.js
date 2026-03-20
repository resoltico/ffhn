import test from 'node:test';
import assert from 'node:assert/strict';

import { fetchUrl, normalizeRequestOptions } from '../lib/services/fetcher.js';
import { targetFailure } from '../lib/utils/error.js';
import { ERROR_TYPES } from '../lib/core/constants.js';

function withMockFetch(mockImpl, fn) {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mockImpl;
    return Promise.resolve()
        .then(fn)
        .finally(() => {
            globalThis.fetch = originalFetch;
        });
}

function makeResponse(status, statusText, body, url = 'https://example.com/final') {
    return {
        ok: status >= 200 && status < 300,
        status,
        statusText,
        url,
        async text() {
            return body;
        }
    };
}

test('fetchUrl returns response details on success', async () => {
    await withMockFetch(async () => makeResponse(200, 'OK', 'hello'), async () => {
        const response = await fetchUrl('https://example.com');
        assert.equal(response.status, 200);
        assert.equal(response.url, 'https://example.com/final');
        assert.equal(response.content, 'hello');
        assert.equal(response.contentBytes, 5);
        assert.equal(response.attempts, 1);
    });

    await withMockFetch(async () => makeResponse(200, 'OK', 'hello', ''), async () => {
        const response = await fetchUrl('https://example.com/original');
        assert.equal(response.url, 'https://example.com/original');
    });
});

test('fetchUrl retries retryable server failures and eventually succeeds', async () => {
    let calls = 0;

    await withMockFetch(async () => {
        calls++;
        if (calls === 1) {
            return makeResponse(503, 'Unavailable', 'bad');
        }
        return makeResponse(200, 'OK', 'good');
    }, async () => {
        const response = await fetchUrl('https://example.com/retry', {
            maxAttempts: 2,
            retryDelayMs: 1
        });

        assert.equal(response.content, 'good');
        assert.equal(response.attempts, 2);
    });
});

test('fetchUrl does not retry client errors or empty bodies', async () => {
    await withMockFetch(async () => makeResponse(404, 'Not Found', 'missing'), async () => {
        await assert.rejects(
            fetchUrl('https://example.com/missing', { maxAttempts: 3, retryDelayMs: 1 }),
            /HTTP 404 Not Found/
        );
    });

    let rateLimitCalls = 0;
    await withMockFetch(async () => {
        rateLimitCalls++;
        return makeResponse(429, 'Too Many Requests', 'slow down');
    }, async () => {
        await assert.rejects(
            fetchUrl('https://example.com/rate-limited', { maxAttempts: 3, retryDelayMs: 1 }),
            /HTTP 429 Too Many Requests/
        );
        assert.equal(rateLimitCalls, 1);
    });

    await withMockFetch(async () => makeResponse(200, 'OK', ''), async () => {
        await assert.rejects(
            fetchUrl('https://example.com/empty', { maxAttempts: 2 }),
            /empty response body/
        );
    });
});

test('fetchUrl retries timeout and transport failures', async () => {
    let timeoutCalls = 0;
    await withMockFetch((_, options) => new Promise((resolve, reject) => {
        timeoutCalls++;
        if (timeoutCalls === 1) {
            options.signal.addEventListener('abort', () => {
                const error = new Error('aborted');
                error.name = 'AbortError';
                reject(error);
            });
            return;
        }

        resolve(makeResponse(200, 'OK', 'late'));
    }), async () => {
        const response = await fetchUrl('https://example.com/timeout', {
            timeoutMs: 5,
            maxAttempts: 2,
            retryDelayMs: 1
        });
        assert.equal(response.content, 'late');
        assert.equal(response.attempts, 2);
    });

    let transportCalls = 0;
    await withMockFetch(async () => {
        transportCalls++;
        if (transportCalls === 1) {
            throw 'socket down';
        }
        return makeResponse(200, 'OK', 'recovered');
    }, async () => {
        const response = await fetchUrl('https://example.com/socket', {
            maxAttempts: 2,
            retryDelayMs: 1
        });
        assert.equal(response.content, 'recovered');
    });
});

test('fetchUrl preserves explicit ffhn failures thrown by fetch', async () => {
    await withMockFetch(async () => {
        throw targetFailure('FETCH_HTTP_ERROR', ERROR_TYPES.NETWORK, 'explicit');
    }, async () => {
        await assert.rejects(
            fetchUrl('https://example.com/explicit', { maxAttempts: 1 }),
            /explicit/
        );
    });
});

test('normalizeRequestOptions validates request settings', () => {
    const normalized = normalizeRequestOptions({
        timeout: 10,
        retryCount: 2,
        retryDelay: 1,
        userAgent: 'ffhn/test'
    });

    assert.equal(normalized.timeoutMs, 10);
    assert.equal(normalized.maxAttempts, 2);
    assert.equal(normalized.retryDelayMs, 1);

    assert.throws(() => normalizeRequestOptions({ timeoutMs: 0 }), /timeoutMs must be a positive integer/);
    assert.throws(() => normalizeRequestOptions({ maxAttempts: 0 }), /maxAttempts must be a positive integer/);
    assert.throws(() => normalizeRequestOptions({ retryDelayMs: -1 }), /retryDelayMs must be a non-negative integer/);
    assert.throws(() => normalizeRequestOptions({ userAgent: ' ' }), /userAgent must be a non-empty string/);
});
