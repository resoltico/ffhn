import { Buffer } from 'node:buffer';
import { hostname } from 'node:os';
import { STATUS, DEFAULTS, ERROR_TYPES, SCHEMA_VERSION } from './constants.js';
import { VERSION } from './version.js';
import { fetchUrl } from '../services/fetcher.js';
import { processWithHtmlcut } from '../adapters/htmlcut.js';
import { computeHash, compareHashes } from '../services/hasher.js';
import { readState, writeState, recordSuccess, recordFailure } from '../services/state.js';
import { readTextFileIfExists, writeTextFileAtomic } from '../adapters/filesystem.js';
import { normalizeError } from '../utils/error.js';
import { validatePositiveInteger } from '../utils/validation.js';

const EMPTY_TIMING = Object.freeze({
    request_ms: 0,
    extract_ms: 0,
    total_ms: 0
});

export async function processSingleTarget(target) {
    const startedAt = Date.now();
    const timings = {
        request_ms: 0,
        extract_ms: 0,
        total_ms: 0
    };
    let state;
    let response = null;
    let extractedContent = null;
    let existingCurrentContent = null;
    let currentHash = null;
    let previousHash = null;

    try {
        state = await readState(target.paths.state);

        const requestStartedAt = Date.now();
        try {
            response = await fetchUrl(target.url, target.request);
        } finally {
            timings.request_ms = Date.now() - requestStartedAt;
        }

        const extractStartedAt = Date.now();
        try {
            extractedContent = await processWithHtmlcut(response.content, target.extract, {
                targetName: target.name,
                timeoutMs: target.request.timeoutMs,
                baseUrl: target.url
            });
        } finally {
            timings.extract_ms = Date.now() - extractStartedAt;
        }

        currentHash = computeHash(extractedContent);
        existingCurrentContent = await readTextFileIfExists(target.paths.current);
        previousHash = state.current_hash ?? (existingCurrentContent === null ? null : computeHash(existingCurrentContent));
        const status = resolveStatus(currentHash, previousHash);

        if (status === STATUS.CHANGED && existingCurrentContent !== null) {
            await writeTextFileAtomic(target.paths.previous, existingCurrentContent);
        }

        await writeTextFileAtomic(target.paths.current, extractedContent);
        recordSuccess(state, currentHash, status);
        await writeState(target.paths.state, state);

        timings.total_ms = Date.now() - startedAt;

        return {
            name: target.name,
            status,
            url: target.url,
            request: buildRequestDetailsFromResponse(response, timings.request_ms),
            extract: buildExtractDetails(extractedContent, timings.extract_ms),
            content: buildContentDetails(extractedContent, existingCurrentContent, status),
            change: buildChangeDetails(extractedContent, existingCurrentContent),
            hash: buildHashDetails(currentHash, previousHash),
            files: buildFiles(target.paths),
            state,
            timing: timings
        };
    } catch (error) {
        const normalizedError = normalizeError(error);
        const persistedState = await persistFailureState(target, state, normalizedError);

        timings.total_ms = Date.now() - startedAt;

        return {
            name: target.name,
            status: STATUS.FAILED,
            url: target.url,
            request: buildRequestDetails(response, normalizedError, timings.request_ms),
            extract: buildFailureExtractDetails(extractedContent, timings.extract_ms),
            content: buildFailureContentDetails(extractedContent, existingCurrentContent),
            hash: buildFailureHashDetails(currentHash, previousHash),
            error: {
                code: normalizedError.code || 'TARGET_FAILED',
                type: normalizedError.type || ERROR_TYPES.INTERNAL,
                message: normalizedError.message,
                details: normalizedError.details || null
            },
            files: buildFiles(target.paths),
            state: persistedState,
            timing: timings
        };
    }
}

export async function processMultipleTargets(targets, concurrency = DEFAULTS.CONCURRENCY) {
    validatePositiveInteger(concurrency, 'concurrency');

    if (targets.length === 0) {
        return [];
    }

    const results = new Array(targets.length);
    let cursor = 0;
    const workerCount = Math.min(concurrency, targets.length);

    await Promise.all(Array.from({ length: workerCount }, async () => {
        while (cursor < targets.length) {
            const index = cursor;
            cursor++;
            results[index] = await processSingleTarget(targets[index]);
        }
    }));

    return results;
}

export function createTargetLoadFailure(discoveredTarget, error) {
    const normalizedError = normalizeError(error);

    return {
        name: discoveredTarget.name,
        status: STATUS.FAILED,
        url: null,
        error: {
            code: normalizedError.code || 'TARGET_LOAD_FAILED',
            type: normalizedError.type || ERROR_TYPES.CONFIG,
            message: normalizedError.message,
            details: normalizedError.details || null
        },
        files: {
            directory: discoveredTarget.dir,
            config: discoveredTarget.configPath,
            state: `${discoveredTarget.dir}/${DEFAULTS.STATE_FILE}`,
            current: `${discoveredTarget.dir}/${DEFAULTS.CURRENT_FILE}`,
            previous: `${discoveredTarget.dir}/${DEFAULTS.PREVIOUS_FILE}`
        },
        state: null,
        timing: EMPTY_TIMING
    };
}

export function buildRunPayload(results, startedAt, finishedAt, watchlistPath, options = {}) {
    return {
        ffhn_version: VERSION,
        schema_version: SCHEMA_VERSION,
        host: hostname(),
        started_at: new Date(startedAt).toISOString(),
        finished_at: new Date(finishedAt).toISOString(),
        duration_ms: finishedAt - startedAt,
        watchlist: watchlistPath,
        selection: {
            target: options.target ?? null,
            concurrency: options.concurrency ?? DEFAULTS.CONCURRENCY
        },
        targets: results,
        summary: summarizeResults(results)
    };
}

export function summarizeResults(results) {
    const summary = {
        total_targets: results.length,
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
    };

    for (const result of results) {
        if (result.status === STATUS.INITIALIZED) {
            summary.initialized++;
            if (result.name) {
                summary.initialized_target_names.push(result.name);
            }
        } else if (result.status === STATUS.CHANGED) {
            summary.changed++;
            if (result.name) {
                summary.changed_target_names.push(result.name);
            }
        } else if (result.status === STATUS.UNCHANGED) {
            summary.unchanged++;
        } else if (result.status === STATUS.FAILED) {
            summary.failed++;
            if (result.name) {
                summary.failed_target_names.push(result.name);
            }
            if (result.error?.type) {
                summary.failure_types[result.error.type] = (summary.failure_types[result.error.type] || 0) + 1;
            }
        }

        summary.total_target_duration_ms += result.timing?.total_ms || 0;
    }

    const successfulTargets = summary.initialized + summary.changed + summary.unchanged;
    summary.successful_targets = successfulTargets;
    summary.success_rate = summary.total_targets === 0
        ? 0
        : Number((successfulTargets / summary.total_targets).toFixed(3));
    summary.avg_target_duration_ms = summary.total_targets === 0
        ? 0
        : Math.round(summary.total_target_duration_ms / summary.total_targets);
    summary.attention_required = summary.changed > 0 || summary.failed > 0;

    return summary;
}

function resolveStatus(currentHash, previousHash) {
    if (previousHash === null) {
        return STATUS.INITIALIZED;
    }

    return compareHashes(currentHash, previousHash)
        ? STATUS.CHANGED
        : STATUS.UNCHANGED;
}

async function persistFailureState(target, state, error) {
    if (!target?.paths?.state || !state) {
        return state ?? null;
    }

    try {
        recordFailure(state, error);
        await writeState(target.paths.state, state);
        return state;
    } catch {
        return state;
    }
}

/* node:coverage disable */
export function buildRequestDetails(response, error, durationMs) {
    if (response) {
        return buildRequestDetailsFromResponse(response, durationMs);
    }

    if (!error?.details || typeof error.details !== 'object') {
        return null;
    }

    const {
        url = null,
        status = null,
        status_text: statusText = null,
        attempt = null
    } = error.details;

    if (![url, status, statusText, attempt].some((value) => value !== null)) {
        return null;
    }

    return {
        final_url: url,
        http_status: status,
        status_text: statusText,
        attempts: attempt,
        body_bytes: null,
        duration_ms: durationMs
    };
}
/* node:coverage enable */

export function buildRequestDetailsFromResponse(response, durationMs) {
    return {
        final_url: response.url,
        http_status: response.status,
        status_text: response.statusText,
        attempts: response.attempts,
        body_bytes: response.contentBytes,
        duration_ms: durationMs
    };
}

export function buildExtractDetails(content, durationMs) {
    return {
        output_bytes: content === null ? null : Buffer.byteLength(content),
        duration_ms: durationMs
    };
}

export function buildContentDetails(currentContent, previousContent, status) {
    const currentChars = currentContent.length;
    const previousChars = previousContent === null ? null : previousContent.length;
    const previews = buildPreviewPair(currentContent, previousContent);

    return {
        current_chars: currentChars,
        previous_chars: previousChars,
        delta_chars: previousChars === null ? null : currentChars - previousChars,
        current_preview: previews.current,
        previous_preview: status === STATUS.INITIALIZED ? null : previews.previous
    };
}

export function buildFailureExtractDetails(content, durationMs) {
    if (content === null && durationMs === 0) {
        return null;
    }

    return buildExtractDetails(content, durationMs);
}

export function buildFailureContentDetails(currentContent, previousContent) {
    if (currentContent === null && previousContent === null) {
        return null;
    }

    const previews = buildPreviewPair(currentContent, previousContent);
    const currentChars = currentContent === null ? null : currentContent.length;
    const previousChars = previousContent === null ? null : previousContent.length;

    return {
        current_chars: currentChars,
        previous_chars: previousChars,
        delta_chars: currentChars === null || previousChars === null ? null : currentChars - previousChars,
        current_preview: previews.current,
        previous_preview: previews.previous
    };
}

export function buildChangeDetails(currentContent, previousContent) {
    if (currentContent === null || previousContent === null) {
        return null;
    }

    const currentLines = extractMeaningfulLines(currentContent);
    const previousLines = extractMeaningfulLines(previousContent);
    const addedItems = diffLines(currentLines, previousLines);
    const removedItems = diffLines(previousLines, currentLines);

    return {
        mode: 'line',
        added: buildChangePreview(addedItems),
        removed: buildChangePreview(removedItems)
    };
}

export function buildHashDetails(currentHash, previousHash) {
    return {
        current: currentHash,
        previous: previousHash,
        algorithm: DEFAULTS.HASH_ALGORITHM
    };
}

export function buildFailureHashDetails(currentHash, previousHash) {
    if (currentHash === null) {
        return null;
    }

    return buildHashDetails(currentHash, previousHash);
}

export function buildFiles(paths) {
    return {
        directory: paths.dir,
        config: paths.config,
        state: paths.state,
        current: paths.current,
        previous: paths.previous
    };
}

function buildPreviewPair(currentContent, previousContent) {
    return {
        current: currentContent === null ? null : createPreview(currentContent),
        previous: previousContent === null ? null : createPreview(previousContent)
    };
}

function createPreview(content) {
    const lines = extractMeaningfulLines(content);

    return {
        text: content.slice(0, DEFAULTS.PREVIEW_CHARS),
        truncated: content.length > DEFAULTS.PREVIEW_CHARS,
        lines: lines.slice(0, DEFAULTS.PREVIEW_LINES),
        total_lines: lines.length,
        lines_truncated: lines.length > DEFAULTS.PREVIEW_LINES
    };
}

function extractMeaningfulLines(content) {
    return content
        .split(/\r?\n/u)
        .map((line) => line.trim())
        .filter((line) => line.length > 0);
}

function diffLines(sourceLines, comparisonLines) {
    const comparisonCounts = countLines(comparisonLines);
    const diff = [];

    for (const line of sourceLines) {
        const remainingCount = comparisonCounts.get(line) || 0;
        if (remainingCount > 0) {
            comparisonCounts.set(line, remainingCount - 1);
            continue;
        }

        diff.push(line);
    }

    return diff;
}

function countLines(lines) {
    const counts = new Map();

    for (const line of lines) {
        counts.set(line, (counts.get(line) || 0) + 1);
    }

    return counts;
}

function buildChangePreview(items) {
    return {
        count: items.length,
        items: items.slice(0, DEFAULTS.CHANGE_PREVIEW_ITEMS),
        truncated: items.length > DEFAULTS.CHANGE_PREVIEW_ITEMS
    };
}
