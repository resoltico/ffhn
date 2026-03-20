import { LIMITS } from '../core/constants.js';

export function validateUrl(url) {
    if (typeof url !== 'string' || url.trim() === '') {
        throw new Error('URL must be a non-empty string');
    }

    let parsedUrl;
    try {
        parsedUrl = new URL(url);
    } catch {
        throw new Error(`Invalid URL: ${url}`);
    }

    if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
        throw new Error(`URL must use HTTP or HTTPS protocol: ${url}`);
    }
}

export function validateTargetName(name) {
    if (typeof name !== 'string' || name.trim() === '') {
        throw new Error('Target name must be a non-empty string');
    }

    if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(name)) {
        throw new Error(`Invalid target name: ${name}. Use letters, numbers, dots, underscores, and hyphens only.`);
    }

    if (name.length > LIMITS.MAX_NAME_LENGTH) {
        throw new Error(`Target name too long: ${name}. Maximum ${LIMITS.MAX_NAME_LENGTH} characters.`);
    }
}

export function validateTargetConfig(config) {
    assertRecord(config, 'Configuration');
    assertAllowedKeys(config, ['target', 'extract', 'request'], 'Configuration');

    if (!Object.hasOwn(config, 'target')) {
        throw new Error('Configuration must have a [target] section');
    }
    if (!Object.hasOwn(config, 'extract')) {
        throw new Error('Configuration must have an [extract] section');
    }

    validateTargetSection(config.target);
    validateExtractSection(config.extract);

    if (Object.hasOwn(config, 'request')) {
        validateRequestSection(config.request);
    }
}

export function validateTargetSection(target) {
    assertRecord(target, '[target]');
    assertAllowedKeys(target, ['url'], '[target]');

    if (!Object.hasOwn(target, 'url')) {
        throw new Error('[target].url is required');
    }

    validateUrl(target.url);
}

export function validateExtractSection(extract) {
    assertRecord(extract, '[extract]');
    assertAllowedKeys(extract, ['from', 'to', 'pattern', 'flags', 'capture', 'all'], '[extract]');

    if (typeof extract.from !== 'string' || extract.from.length === 0) {
        throw new Error('[extract].from must be a non-empty string');
    }

    if (typeof extract.to !== 'string' || extract.to.length === 0) {
        throw new Error('[extract].to must be a non-empty string');
    }

    if (Object.hasOwn(extract, 'pattern') && !['literal', 'regex'].includes(extract.pattern)) {
        throw new Error('[extract].pattern must be "literal" or "regex"');
    }

    if (Object.hasOwn(extract, 'flags')) {
        if (typeof extract.flags !== 'string' || extract.flags.trim() === '') {
            throw new Error('[extract].flags must be a non-empty string');
        }

        if ((extract.pattern ?? 'literal') !== 'regex') {
            throw new Error('[extract].flags can only be used when [extract].pattern is "regex"');
        }
    }

    if (Object.hasOwn(extract, 'capture') && !['inner', 'outer'].includes(extract.capture)) {
        throw new Error('[extract].capture must be "inner" or "outer"');
    }

    if (Object.hasOwn(extract, 'all') && typeof extract.all !== 'boolean') {
        throw new Error('[extract].all must be boolean');
    }
}

export function validateRequestSection(request) {
    assertRecord(request, '[request]');
    assertAllowedKeys(request, ['timeout_ms', 'max_attempts', 'retry_delay_ms', 'user_agent'], '[request]');

    if (Object.hasOwn(request, 'timeout_ms')) {
        validatePositiveInteger(request.timeout_ms, '[request].timeout_ms');
    }

    if (Object.hasOwn(request, 'max_attempts')) {
        validatePositiveInteger(request.max_attempts, '[request].max_attempts');
    }

    if (Object.hasOwn(request, 'retry_delay_ms')) {
        validateNonNegativeInteger(request.retry_delay_ms, '[request].retry_delay_ms');
    }

    if (Object.hasOwn(request, 'user_agent')) {
        if (typeof request.user_agent !== 'string' || request.user_agent.trim() === '') {
            throw new Error('[request].user_agent must be a non-empty string');
        }
    }
}

export function validatePositiveInteger(value, label) {
    if (!Number.isInteger(value) || value <= 0) {
        throw new Error(`${label} must be a positive integer`);
    }
}

export function validateNonNegativeInteger(value, label) {
    if (!Number.isInteger(value) || value < 0) {
        throw new Error(`${label} must be a non-negative integer`);
    }
}

function assertAllowedKeys(record, allowedKeys, label) {
    const allowedKeySet = new Set(allowedKeys);
    for (const key of Object.keys(record)) {
        if (!allowedKeySet.has(key)) {
            throw new Error(`${label} contains unsupported key: ${key}`);
        }
    }
}

function assertRecord(value, label) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw new Error(`${label} must be an object`);
    }
}
