import test from 'node:test';
import assert from 'node:assert/strict';

import {
    validateUrl,
    validateTargetName,
    validateTargetConfig,
    validateTargetSection,
    validateExtractSection,
    validateRequestSection,
    validatePositiveInteger,
    validateNonNegativeInteger
} from '../lib/utils/validation.js';

test('validateUrl accepts http/https and rejects invalid values', () => {
    validateUrl('https://example.com');
    validateUrl('http://example.com/path');

    assert.throws(() => validateUrl(''), /non-empty string/);
    assert.throws(() => validateUrl('notaurl'), /Invalid URL/);
    assert.throws(() => validateUrl('ftp://example.com'), /HTTP or HTTPS/);
});

test('validateTargetName enforces a strict filesystem-safe format', () => {
    validateTargetName('abc_123.test-name');

    assert.throws(() => validateTargetName(''), /non-empty string/);
    assert.throws(() => validateTargetName('-bad'), /Invalid target name/);
    assert.throws(() => validateTargetName(`a${'b'.repeat(255)}`), /Target name too long/);
});

test('validateTargetSection, validateExtractSection, and validateRequestSection reject unsupported keys', () => {
    validateTargetSection({ url: 'https://example.com' });
    validateExtractSection({
        from: '<main>',
        to: '</main>',
        pattern: 'regex',
        flags: 'iu',
        capture: 'outer',
        all: true
    });
    validateRequestSection({ timeout_ms: 1000, max_attempts: 2, retry_delay_ms: 0, user_agent: 'ffhn/test' });

    assert.throws(() => validateTargetSection({}), /\[target\]\.url is required/);
    assert.throws(() => validateTargetSection({ url: 'https://example.com', extra: true }), /unsupported key/);
    assert.throws(() => validateExtractSection({ from: '', to: '</main>' }), /non-empty string/);
    assert.throws(() => validateExtractSection({ from: '<main>' }), /\[extract\].to/);
    assert.throws(() => validateExtractSection({ from: '<main>', to: '</main>', pattern: 'glob' }), /literal" or "regex/);
    assert.throws(() => validateExtractSection({ from: '<main>', to: '</main>', pattern: 'regex', flags: ' ' }), /non-empty string/);
    assert.throws(() => validateExtractSection({ from: '<main>', to: '</main>', flags: 'i' }), /only be used when \[extract\]\.pattern is "regex"/);
    assert.throws(() => validateExtractSection({ from: '<main>', to: '</main>', capture: 'middle' }), /"inner" or "outer"/);
    assert.throws(() => validateExtractSection({ from: '<main>', to: '</main>', all: 'yes' }), /must be boolean/);
    assert.throws(() => validateRequestSection({ timeout_ms: 0 }), /positive integer/);
    assert.throws(() => validateRequestSection({ retry_delay_ms: -1 }), /non-negative integer/);
    assert.throws(() => validateRequestSection({ user_agent: '  ' }), /non-empty string/);
});

test('validateTargetConfig requires the new top-level schema', () => {
    validateTargetConfig({
        target: { url: 'https://example.com' },
        extract: { from: '<main>', to: '</main>' },
        request: { timeout_ms: 1000 }
    });

    assert.throws(() => validateTargetConfig(null), /must be an object/);
    assert.throws(() => validateTargetConfig({}), /\[target\]/);
    assert.throws(() => validateTargetConfig({ target: { url: 'https://example.com' } }), /\[extract\]/);
    assert.throws(
        () => validateTargetConfig({
            target: { url: 'https://example.com' },
            extract: { from: '<main>', to: '</main>' },
            htmlcut: {}
        }),
        /unsupported key/
    );
    assert.throws(
        () => validateTargetConfig({
            target: { url: 'https://example.com' },
            extract: { start: '<main>', end: '</main>' }
        }),
        /unsupported key: start/
    );
});

test('validate integer helpers are strict', () => {
    validatePositiveInteger(1, 'value');
    validateNonNegativeInteger(0, 'value');

    assert.throws(() => validatePositiveInteger(0, 'value'), /positive integer/);
    assert.throws(() => validateNonNegativeInteger(-1, 'value'), /non-negative integer/);
});
