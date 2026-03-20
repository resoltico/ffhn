import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { readFile, writeFile } from 'node:fs/promises';

import pkg from '../package.json' with { type: 'json' };
import { readJsonFile, withTempDir, createFakeHtmlcutBin } from './helpers.js';
import { computeHash } from '../lib/services/hasher.js';

const CLI_PATH = join(process.cwd(), 'bin', 'ffhn.js');

function runCli(args, cwd, options = {}) {
    return new Promise((resolve) => {
        const child = spawn(process.execPath, [CLI_PATH, ...args], {
            cwd,
            env: {
                ...process.env,
                ...(options.env || {})
            }
        });

        let stdout = '';
        let stderr = '';

        child.stdout.on('data', (chunk) => {
            stdout += chunk.toString();
        });

        child.stderr.on('data', (chunk) => {
            stderr += chunk.toString();
        });

        child.on('close', (code) => {
            resolve({ code, stdout, stderr });
        });
    });
}

async function setTargetUrl(cwd, targetName, url) {
    const configPath = join(cwd, 'watchlist', targetName, 'target.toml');
    const targetToml = await readFile(configPath, 'utf8');
    await writeFile(
        configPath,
        targetToml.replace(/url = ".*"/, `url = "${url}"`),
        'utf8'
    );
}

async function createFetchMockModule(cwd, responseBody = '<main>hello</main>') {
    const mockPath = join(cwd, 'fetch-mock.mjs');
    await writeFile(mockPath, `globalThis.fetch = async () => ({
  ok: true,
  status: 200,
  statusText: 'OK',
  url: 'https://example.com/final',
  async text() {
    return ${JSON.stringify(responseBody)};
  }
});
`, 'utf8');
    return mockPath;
}

test('cli version output reflects package version', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const version = await runCli(['--version'], cwd);
        assert.equal(version.code, 0);
        const payload = JSON.parse(version.stdout.trim());
        assert.equal(payload.command, 'version');
        assert.equal(payload.ffhn_version, pkg.version);
    });
});

test('cli accepts explicit json mode and rejects unknown options as usage errors', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const jsonStatus = await runCli(['status', '--json'], cwd);
        assert.equal(jsonStatus.code, 0);
        const jsonPayload = JSON.parse(jsonStatus.stdout.trim());
        assert.equal(jsonPayload.command, 'status');

        const unknownOption = await runCli(['--wat'], cwd);
        assert.equal(unknownOption.code, 1);
        const unknownPayload = JSON.parse(unknownOption.stderr.trim());
        assert.equal(unknownPayload.error.code, 'UNKNOWN_OPTION');
        assert.equal(unknownPayload.error.details.parse_error_code, 'ERR_PARSE_ARGS_UNKNOWN_OPTION');
    });
});

test('cli supports positional help/version and rejects bad command permutations', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const helpResult = await runCli(['help'], cwd);
        assert.equal(helpResult.code, 0);
        const helpPayload = JSON.parse(helpResult.stdout.trim());
        assert.equal(helpPayload.command, 'help');
        assert.equal(helpPayload.topic, undefined);

        const runHelpResult = await runCli(['help', 'run'], cwd);
        assert.equal(runHelpResult.code, 0);
        const runHelpPayload = JSON.parse(runHelpResult.stdout.trim());
        assert.equal(runHelpPayload.command, 'help');
        assert.equal(runHelpPayload.topic, 'run');
        assert.equal(runHelpPayload.usage, 'ffhn run [options]');

        const versionResult = await runCli(['version'], cwd);
        assert.equal(versionResult.code, 0);
        assert.equal(JSON.parse(versionResult.stdout.trim()).command, 'version');

        const conflictingResult = await runCli(['help', '--version'], cwd);
        assert.equal(conflictingResult.code, 1);
        assert.equal(JSON.parse(conflictingResult.stderr.trim()).error.code, 'CONFLICTING_COMMANDS');

        const extraPositional = await runCli(['run', 'extra'], cwd);
        assert.equal(extraPositional.code, 1);
        assert.equal(JSON.parse(extraPositional.stderr.trim()).error.code, 'UNEXPECTED_ARGUMENTS');

        const unknownHelpTopic = await runCli(['help', 'wat'], cwd);
        assert.equal(unknownHelpTopic.code, 1);
        assert.equal(JSON.parse(unknownHelpTopic.stderr.trim()).error.code, 'UNKNOWN_COMMAND');
    });
});

test('cli accepts help/version flags alongside operational commands', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const helpResult = await runCli(['run', '--help', '--watchlist', '/tmp/demo-watchlist'], cwd);
        assert.equal(helpResult.code, 0);
        const helpPayload = JSON.parse(helpResult.stdout.trim());
        assert.equal(helpPayload.command, 'help');
        assert.equal(helpPayload.topic, 'run');
        assert.equal(helpPayload.usage, 'ffhn run [options]');

        const versionResult = await runCli(['--version', 'status', '--target', 'demo'], cwd);
        assert.equal(versionResult.code, 0);
        assert.equal(JSON.parse(versionResult.stdout.trim()).command, 'version');

        const versionHelpResult = await runCli(['--help', 'version'], cwd);
        assert.equal(versionHelpResult.code, 0);
        const versionHelpPayload = JSON.parse(versionHelpResult.stdout.trim());
        assert.equal(versionHelpPayload.topic, 'version');
        assert.equal(versionHelpPayload.usage, 'ffhn version [options]');

        const unsupported = await runCli(['init', '--help', '--concurrency', '2'], cwd);
        assert.equal(unsupported.code, 1);
        assert.equal(JSON.parse(unsupported.stderr.trim()).error.code, 'UNSUPPORTED_OPTION');
    });
});

test('cli init requires explicit target name', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const result = await runCli(['init'], cwd);
        assert.equal(result.code, 1);
        const payload = JSON.parse(result.stderr.trim());
        assert.equal(payload.error.code, 'TARGET_REQUIRED');
    });
});

test('cli rejects malformed target selectors as usage errors', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const initResult = await runCli(['init', '--target', 'bad/name'], cwd);
        assert.equal(initResult.code, 1);
        const initPayload = JSON.parse(initResult.stderr.trim());
        assert.equal(initPayload.error.code, 'INVALID_TARGET');

        const statusResult = await runCli(['status', '--target', 'bad/name'], cwd);
        assert.equal(statusResult.code, 1);
        const statusPayload = JSON.parse(statusResult.stderr.trim());
        assert.equal(statusPayload.error.code, 'INVALID_TARGET');
    });
});

test('cli surfaces TOML parse diagnostics for broken target configs', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        assert.equal((await runCli(['init', '--target', 'demo'], cwd)).code, 0);

        await writeFile(
            join(cwd, 'watchlist', 'demo', 'target.toml'),
            '[target]\nurl = "https://example.com"\n\n[extract\nfrom = "<main>"\nto = "</main>"\n',
            'utf8'
        );

        const result = await runCli(['run', '--target', 'demo'], cwd);
        assert.equal(result.code, 2);
        const payload = JSON.parse(result.stdout.trim());
        assert.equal(payload.targets[0].error.code, 'TARGET_CONFIG_PARSE_FAILED');
        assert.match(payload.targets[0].error.message, /line 4, column 2/);
        assert.equal(payload.targets[0].error.details.line, 4);
        assert.equal(payload.targets[0].error.details.column, 2);
        assert.match(payload.targets[0].error.details.parse_summary, /incomplete key-value/);
        assert.match(payload.targets[0].error.details.code_frame, /4:\s+\[extract/);
    });
});

test('cli init writes new schema and status reports targets', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const initResult = await runCli(['init', '--target', 'demo'], cwd);
        assert.equal(initResult.code, 0);
        const payload = JSON.parse(initResult.stdout.trim());
        assert.equal(payload.target.name, 'demo');

        const configPath = join(cwd, 'watchlist', 'demo', 'target.toml');
        const targetToml = await readFile(configPath, 'utf8');
        assert.match(targetToml, /\[target\]/);
        assert.ok(targetToml.includes('from = "<main\\\\b[^>]*>"'));
        assert.ok(targetToml.includes('pattern = "regex"'));
        assert.doesNotMatch(targetToml, /name =/);

        const status = await runCli(['status'], cwd);
        assert.equal(status.code, 0);
        const statusPayload = JSON.parse(status.stdout.trim());
        assert.equal(statusPayload.count, 1);
        assert.equal(statusPayload.targets[0].status, 'pending');
        assert.equal(statusPayload.summary.pending, 1);

        const filteredStatus = await runCli(['status', '--target', 'demo'], cwd);
        assert.equal(filteredStatus.code, 0);
        const filteredStatusPayload = JSON.parse(filteredStatus.stdout.trim());
        assert.equal(filteredStatusPayload.count, 1);
        assert.equal(filteredStatusPayload.selection.target, 'demo');
        assert.equal(filteredStatusPayload.targets[0].content, null);

        const duplicateInit = await runCli(['init', '--target', 'demo'], cwd);
        assert.equal(duplicateInit.code, 2);
        const duplicatePayload = JSON.parse(duplicateInit.stderr.trim());
        assert.equal(duplicatePayload.error.code, 'TARGET_EXISTS');
    });
});

test('cli init template extracts clean main content on first run', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        assert.equal((await runCli(['init', '--target', 'demo'], cwd)).code, 0);

        const fakeHtmlcut = await createFakeHtmlcutBin(cwd, 'extract');
        const fetchMock = await createFetchMockModule(cwd, '<html><body><main>hello</main></body></html>');

        const result = await runCli(['run', '--target', 'demo'], cwd, {
            env: {
                PATH: `${fakeHtmlcut.binDir}:${process.env.PATH}`,
                NODE_OPTIONS: `--import ${fetchMock}`
            }
        });

        assert.equal(result.code, 0);
        const payload = JSON.parse(result.stdout.trim());
        assert.equal(payload.targets[0].status, 'initialized');
        assert.equal(payload.targets[0].content.current_preview.text, 'hello');
        assert.deepEqual(payload.targets[0].content.current_preview.lines, ['hello']);
    });
});

test('cli run emits initialized, unchanged, and changed states', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        assert.equal((await runCli(['init', '--target', 'demo'], cwd)).code, 0);

        const fakeHtmlcut = await createFakeHtmlcutBin(cwd, 'success');
        const initialFetchMock = await createFetchMockModule(cwd, '<main>hello</main>');
        await setTargetUrl(cwd, 'demo', 'http://demo.test/path');

        const firstRun = await runCli(['run', '--target', 'demo'], cwd, {
            env: {
                PATH: `${fakeHtmlcut.binDir}:${process.env.PATH}`,
                NODE_OPTIONS: `--import ${initialFetchMock}`
            }
        });
        assert.equal(firstRun.code, 0);
        const firstPayload = JSON.parse(firstRun.stdout.trim());
        assert.equal(firstPayload.targets[0].status, 'initialized');
        assert.deepEqual(firstPayload.selection, {
            target: 'demo',
            concurrency: 4
        });
        assert.equal(firstPayload.summary.initialized_target_names[0], 'demo');
        assert.equal(firstPayload.targets[0].content.current_preview.text, '<MAIN>HELLO</MAIN>');
        assert.deepEqual(firstPayload.targets[0].content.current_preview.lines, ['<MAIN>HELLO</MAIN>']);
        assert.equal(firstPayload.targets[0].content.previous_preview, null);
        assert.equal(firstPayload.targets[0].change, null);

        const secondRun = await runCli(['run', '--target', 'demo'], cwd, {
            env: {
                PATH: `${fakeHtmlcut.binDir}:${process.env.PATH}`,
                NODE_OPTIONS: `--import ${initialFetchMock}`
            }
        });
        assert.equal(secondRun.code, 0);
        const secondPayload = JSON.parse(secondRun.stdout.trim());
        assert.equal(secondPayload.targets[0].status, 'unchanged');
        assert.deepEqual(secondPayload.targets[0].change, {
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

        const changedFetchMock = await createFetchMockModule(cwd, '<main>goodbye</main>');
        const thirdRun = await runCli(['run', '--target', 'demo', '--pretty'], cwd, {
            env: {
                PATH: `${fakeHtmlcut.binDir}:${process.env.PATH}`,
                NODE_OPTIONS: `--import ${changedFetchMock}`
            }
        });
        assert.equal(thirdRun.code, 0);
        assert.match(thirdRun.stdout, /\n  "command": "run"/);
        const thirdPayload = JSON.parse(thirdRun.stdout);
        assert.equal(thirdPayload.targets[0].status, 'changed');
        assert.deepEqual(thirdPayload.summary.changed_target_names, ['demo']);
        assert.equal(thirdPayload.summary.attention_required, true);
        assert.equal(thirdPayload.targets[0].content.delta_chars, 2);
        assert.equal(thirdPayload.targets[0].content.previous_preview.text, '<MAIN>HELLO</MAIN>');
        assert.equal(thirdPayload.targets[0].content.current_preview.text, '<MAIN>GOODBYE</MAIN>');
        assert.deepEqual(thirdPayload.targets[0].content.current_preview.lines, ['<MAIN>GOODBYE</MAIN>']);
        assert.deepEqual(thirdPayload.targets[0].content.previous_preview.lines, ['<MAIN>HELLO</MAIN>']);
        assert.deepEqual(thirdPayload.targets[0].change, {
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

        const state = await readJsonFile(join(cwd, 'watchlist', 'demo', 'state.json'));
        assert.equal(state.stats.runs, 3);
        assert.equal(state.stats.changes, 1);
        assert.equal(state.current_hash, thirdPayload.targets[0].hash.current);

        const statusAfterRun = await runCli(['status', '--target', 'demo'], cwd);
        assert.equal(statusAfterRun.code, 0);
        const statusAfterRunPayload = JSON.parse(statusAfterRun.stdout.trim());
        assert.equal(statusAfterRunPayload.targets[0].status, 'ready');
        assert.equal(statusAfterRunPayload.targets[0].content.current_preview.text, '<MAIN>GOODBYE</MAIN>');
        assert.equal(statusAfterRunPayload.targets[0].content.previous_preview.text, '<MAIN>HELLO</MAIN>');
        assert.deepEqual(statusAfterRunPayload.targets[0].content.current_preview.lines, ['<MAIN>GOODBYE</MAIN>']);
        assert.deepEqual(statusAfterRunPayload.targets[0].content.previous_preview.lines, ['<MAIN>HELLO</MAIN>']);
        assert.equal(statusAfterRunPayload.targets[0].artifacts.current.matches_state_hash, true);
        assert.equal(statusAfterRunPayload.targets[0].artifacts.previous.matches_state_hash, true);
        assert.deepEqual(statusAfterRunPayload.targets[0].change, {
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
        assert.equal(statusAfterRunPayload.summary.invalid_codes.CURRENT_HASH_MISMATCH, undefined);

        await writeFile(join(cwd, 'watchlist', 'demo', 'current.txt'), 'TAMPERED', 'utf8');

        const tamperedStatus = await runCli(['status', '--target', 'demo'], cwd);
        assert.equal(tamperedStatus.code, 2);
        const tamperedPayload = JSON.parse(tamperedStatus.stdout.trim());
        assert.equal(tamperedPayload.targets[0].status, 'invalid');
        assert.equal(tamperedPayload.targets[0].error.code, 'STATUS_ARTIFACTS_INVALID');
        assert.equal(tamperedPayload.targets[0].artifacts.current.hash, computeHash('TAMPERED'));
        assert.equal(tamperedPayload.targets[0].artifacts.current.matches_state_hash, false);
        assert.deepEqual(tamperedPayload.targets[0].change, {
            mode: 'line',
            added: {
                count: 1,
                items: ['TAMPERED'],
                truncated: false
            },
            removed: {
                count: 1,
                items: ['<MAIN>HELLO</MAIN>'],
                truncated: false
            }
        });
        assert.deepEqual(tamperedPayload.targets[0].artifacts.issues, [
            {
                code: 'CURRENT_HASH_MISMATCH',
                message: 'current.txt content does not match state.current_hash'
            }
        ]);
        assert.deepEqual(tamperedPayload.summary.invalid_codes, {
            CURRENT_HASH_MISMATCH: 1
        });
    });
});

test('cli run returns target failure exit code when htmlcut is unavailable', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        assert.equal((await runCli(['init', '--target', 'demo'], cwd)).code, 0);

        const fetchMock = await createFetchMockModule(cwd, '<main>hello</main>');
        await setTargetUrl(cwd, 'demo', 'http://demo.test/path');

        const result = await runCli(['run', '--target', 'demo'], cwd, {
            env: {
                PATH: cwd,
                NODE_OPTIONS: `--import ${fetchMock}`
            }
        });

        assert.equal(result.code, 3);
        const payload = JSON.parse(result.stdout.trim());
        assert.equal(payload.targets[0].status, 'failed');
        assert.equal(payload.targets[0].error.code, 'HTMLCUT_NOT_FOUND');
        assert.deepEqual(payload.summary.failed_target_names, ['demo']);
        assert.deepEqual(payload.summary.failure_types, {
            dependency: 1
        });
        assert.equal(payload.targets[0].content, null);
        assert.equal(payload.targets[0].change, undefined);
    });
});

test('cli rejects malformed concurrency values instead of parsing them loosely', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const result = await runCli(['run', '--concurrency', '4.5'], cwd);
        assert.equal(result.code, 1);
        const payload = JSON.parse(result.stderr.trim());
        assert.equal(payload.error.code, 'INVALID_CONCURRENCY');
    });
});

test('cli rejects unsupported options instead of silently ignoring them', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const statusResult = await runCli(['status', '--concurrency', '2'], cwd);
        assert.equal(statusResult.code, 1);
        assert.equal(JSON.parse(statusResult.stderr.trim()).error.code, 'UNSUPPORTED_OPTION');

        const initResult = await runCli(['init', '--target', 'demo', '--concurrency', '2'], cwd);
        assert.equal(initResult.code, 1);
        assert.equal(JSON.parse(initResult.stderr.trim()).error.code, 'UNSUPPORTED_OPTION');

        const versionResult = await runCli(['--version', '--target', 'demo'], cwd);
        assert.equal(versionResult.code, 1);
        assert.equal(JSON.parse(versionResult.stderr.trim()).error.code, 'UNSUPPORTED_OPTION');
    });
});

test('cli rejects zero concurrency as a usage error', async () => {
    await withTempDir('ffhn-cli-', async (cwd) => {
        const result = await runCli(['run', '--concurrency', '0'], cwd);
        assert.equal(result.code, 1);
        assert.equal(JSON.parse(result.stderr.trim()).error.code, 'INVALID_CONCURRENCY');
    });
});
