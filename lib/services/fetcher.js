import { Buffer } from 'node:buffer';
import { DEFAULTS, HTTP_STATUS, ERROR_TYPES } from '../core/constants.js';
import { targetFailure } from '../utils/error.js';
import { validateNonNegativeInteger, validatePositiveInteger, validateUrl } from '../utils/validation.js';

export async function fetchUrl(url, options = {}) {
    validateUrl(url);

    const requestOptions = normalizeRequestOptions(options);
    let lastError = null;

    for (let attempt = 1; attempt <= requestOptions.maxAttempts; attempt++) {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), requestOptions.timeoutMs);

        try {
            const response = await fetch(url, {
                headers: {
                    'User-Agent': requestOptions.userAgent
                },
                signal: controller.signal
            });
            const content = await response.text();

            if (!response.ok) {
                throw targetFailure(
                    'FETCH_HTTP_ERROR',
                    ERROR_TYPES.NETWORK,
                    `HTTP ${response.status} ${response.statusText} for ${url}`,
                    {
                        url,
                        attempt,
                        status: response.status,
                        status_text: response.statusText
                    }
                );
            }

            if (content.length === 0) {
                throw targetFailure(
                    'FETCH_EMPTY_BODY',
                    ERROR_TYPES.NETWORK,
                    `Received an empty response body from ${url}`,
                    {
                        url,
                        attempt
                    }
                );
            }

            clearTimeout(timeoutId);

            return {
                url: response.url || url,
                status: response.status,
                statusText: response.statusText,
                content,
                contentBytes: Buffer.byteLength(content),
                attempts: attempt
            };
        } catch (error) {
            clearTimeout(timeoutId);
            lastError = normalizeFetchError(error, {
                url,
                attempt,
                timeoutMs: requestOptions.timeoutMs
            });

            if (!shouldRetry(lastError, attempt, requestOptions.maxAttempts)) {
                break;
            }

            await sleep(requestOptions.retryDelayMs * Math.pow(
                DEFAULTS.RETRY_BACKOFF_FACTOR,
                attempt - 1
            ));
        }
    }

    throw lastError;
}

export function normalizeRequestOptions(options = {}) {
    const normalizedOptions = {
        timeoutMs: options.timeoutMs ?? options.timeout ?? DEFAULTS.TIMEOUT_MS,
        maxAttempts: options.maxAttempts ?? options.retryCount ?? DEFAULTS.MAX_ATTEMPTS,
        retryDelayMs: options.retryDelayMs ?? options.retryDelay ?? DEFAULTS.RETRY_DELAY_MS,
        userAgent: options.userAgent ?? DEFAULTS.USER_AGENT
    };

    validatePositiveInteger(normalizedOptions.timeoutMs, 'timeoutMs');
    validatePositiveInteger(normalizedOptions.maxAttempts, 'maxAttempts');
    validateNonNegativeInteger(normalizedOptions.retryDelayMs, 'retryDelayMs');

    if (typeof normalizedOptions.userAgent !== 'string' || normalizedOptions.userAgent.trim() === '') {
        throw new Error('userAgent must be a non-empty string');
    }

    return normalizedOptions;
}

function shouldRetry(error, attempt, maxAttempts) {
    if (attempt >= maxAttempts) {
        return false;
    }

    if (error.code === 'FETCH_TIMEOUT' || error.code === 'FETCH_TRANSPORT_ERROR') {
        return true;
    }

    return error.code === 'FETCH_HTTP_ERROR' && error.details?.status >= HTTP_STATUS.SERVER_ERROR_START;
}

function normalizeFetchError(error, context) {
    if (error instanceof Error && error.name === 'AbortError') {
        return targetFailure(
            'FETCH_TIMEOUT',
            ERROR_TYPES.NETWORK,
            `Request timed out after ${context.timeoutMs}ms for ${context.url}`,
            {
                url: context.url,
                attempt: context.attempt,
                timeout_ms: context.timeoutMs
            },
            error
        );
    }

    if (error instanceof Error && error.code && error.type) {
        return error;
    }

    return targetFailure(
        'FETCH_TRANSPORT_ERROR',
        ERROR_TYPES.NETWORK,
        `Failed to fetch ${context.url}: ${error instanceof Error ? error.message : String(error)}`,
        {
            url: context.url,
            attempt: context.attempt
        },
        error
    );
}

function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
