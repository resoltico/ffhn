import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import {
    buildTomlParseDetails,
    buildTomlParseMessage,
    extractTomlSummary,
    loadTargetConfig,
    normalizeTargetConfig
} from '../lib/core/config.js';
import { createTargetTemplate, loadTarget } from '../lib/core/target.js';
import { DEFAULTS } from '../lib/core/constants.js';
import { withTempDir } from './helpers.js';

const VALID_TOML = `[target]
url = "https://example.com"

[extract]
from = "<main\\\\b[^>]*>"
to = "</main>"
pattern = "regex"
flags = "iu"
capture = "outer"
all = true

[request]
timeout_ms = 1234
max_attempts = 5
retry_delay_ms = 100
user_agent = "ffhn/test"
`;

test('loadTargetConfig parses and normalizes strict target configs', async () => {
    await withTempDir('ffhn-config-', async (baseDir) => {
        const configPath = join(baseDir, 'target.toml');
        await writeFile(configPath, VALID_TOML, 'utf8');

        const config = await loadTargetConfig(configPath);
        assert.equal(config.url, 'https://example.com');
        assert.equal(config.extract.from, '<main\\b[^>]*>');
        assert.equal(config.extract.to, '</main>');
        assert.equal(config.extract.pattern, 'regex');
        assert.equal(config.extract.flags, 'iu');
        assert.equal(config.extract.capture, 'outer');
        assert.equal(config.extract.all, true);
        assert.equal(config.request.timeoutMs, 1234);
        assert.equal(config.request.maxAttempts, 5);
        assert.equal(config.request.retryDelayMs, 100);
        assert.equal(config.request.userAgent, 'ffhn/test');
    });
});

test('normalizeTargetConfig fills request defaults', () => {
    const config = normalizeTargetConfig({
        target: { url: 'https://example.com' },
        extract: { from: '<main\\b[^>]*>', to: '</main>' }
    });

    assert.equal(config.request.timeoutMs, DEFAULTS.TIMEOUT_MS);
    assert.equal(config.request.maxAttempts, DEFAULTS.MAX_ATTEMPTS);
    assert.equal(config.request.retryDelayMs, DEFAULTS.RETRY_DELAY_MS);
    assert.equal(config.request.userAgent, DEFAULTS.USER_AGENT);
});

test('loadTargetConfig surfaces missing, parse, and validation errors', async () => {
    await withTempDir('ffhn-config-', async (baseDir) => {
        await assert.rejects(
            loadTargetConfig(join(baseDir, 'missing.toml')),
            /Target config not found/
        );

        const invalidTomlPath = join(baseDir, 'invalid.toml');
        await writeFile(invalidTomlPath, '[target\nurl = "x"', 'utf8');
        await assert.rejects(async () => {
            try {
                await loadTargetConfig(invalidTomlPath);
            } catch (error) {
                assert.match(error.message, /Failed to parse target config: .* line 1, column 2/);
                assert.equal(error.details.config_path, invalidTomlPath);
                assert.equal(error.details.line, 1);
                assert.equal(error.details.column, 2);
                assert.match(error.details.parse_summary, /incomplete key-value/);
                assert.match(error.details.parse_message, /Invalid TOML document/);
                assert.match(error.details.code_frame, /1:\s+\[target/);
                throw error;
            }
        }, /Failed to parse target config/);

        const invalidConfigPath = join(baseDir, 'bad.toml');
        await writeFile(invalidConfigPath, '[target]\nurl="bad"\n[extract]\nfrom="a"\nto="b"\n', 'utf8');
        await assert.rejects(
            loadTargetConfig(invalidConfigPath),
            /Invalid URL/
        );
    });
});

test('loadTarget maps discovered targets into runtime targets', async () => {
    await withTempDir('ffhn-target-', async (baseDir) => {
        const configPath = join(baseDir, 'target.toml');
        await writeFile(configPath, VALID_TOML, 'utf8');

        const target = await loadTarget({
            name: 'site',
            dir: baseDir,
            configPath
        });

        assert.equal(target.name, 'site');
        assert.equal(target.url, 'https://example.com');
        assert.equal(target.request.timeoutMs, 1234);
        assert.equal(target.extract.to, '</main>');
        assert.equal(target.paths.config, configPath);
        assert.equal(target.paths.state, join(baseDir, 'state.json'));
    });
});

test('TOML parse formatting helpers preserve concise and fallback diagnostics', () => {
    const detailed = buildTomlParseDetails('/tmp/demo.toml', {
        message: 'Invalid TOML document: incomplete key-value\n\n1: bad',
        line: 1,
        column: 4,
        codeblock: '1: bad'
    });
    assert.deepEqual(detailed, {
        config_path: '/tmp/demo.toml',
        line: 1,
        column: 4,
        parse_summary: 'incomplete key-value',
        parse_message: 'Invalid TOML document: incomplete key-value\n\n1: bad',
        code_frame: '1: bad'
    });
    assert.equal(
        buildTomlParseMessage('/tmp/demo.toml', detailed),
        'Failed to parse target config: /tmp/demo.toml at line 1, column 4: incomplete key-value'
    );

    const sparse = buildTomlParseDetails('/tmp/demo.toml', {
        line: 'nope',
        column: undefined,
        codeblock: '',
        toString() {
            return '';
        }
    });
    assert.deepEqual(sparse, {
        config_path: '/tmp/demo.toml',
        line: null,
        column: null,
        parse_summary: null,
        parse_message: '',
        code_frame: null
    });
    assert.equal(
        buildTomlParseMessage('/tmp/demo.toml', sparse),
        'Failed to parse target config: /tmp/demo.toml'
    );

    assert.equal(extractTomlSummary('plain parser message'), 'plain parser message');
    assert.equal(extractTomlSummary(''), null);
});

test('createTargetTemplate emits the new strict schema', () => {
    const template = createTargetTemplate();
    assert.match(template, /\[target\]/);
    assert.match(template, /\[extract\]/);
    assert.match(template, /\[request\]/);
    assert.ok(template.includes('from = "<main\\\\b[^>]*>"'));
    assert.ok(template.includes('pattern = "regex"'));
    assert.doesNotMatch(template, /name =/);
});
