import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFile, chmod } from 'node:fs/promises';
import { join } from 'node:path';

import { executeCommand, executeHtmlcut } from '../lib/adapters/process.js';
import { processWithHtmlcut } from '../lib/adapters/htmlcut.js';
import { withTempDir, createFakeHtmlcutBin, withPatchedPath, withExactPath } from './helpers.js';

test('executeCommand handles success, failure, timeout, and missing commands', async () => {
    const success = await executeCommand('node', ['-e', 'process.stdout.write("ok")']);
    assert.equal(success.stdout, 'ok');

    const withInput = await executeCommand(
        'node',
        ['-e', 'process.stdin.on("data", chunk => process.stdout.write(chunk))'],
        { input: 'hello' }
    );
    assert.equal(withInput.stdout, 'hello');

    await assert.rejects(
        executeCommand('node', ['-e', 'process.stderr.write("boom"); process.exit(2)']),
        /failed/
    );

    await assert.rejects(
        executeCommand('node', ['-e', 'setTimeout(() => {}, 1000)'], { timeoutMs: 10 }),
        /timed out/
    );

    await assert.rejects(
        executeCommand(`missing-command-${Date.now()}`),
        /Required command not found/
    );

    await withTempDir('ffhn-process-', async (baseDir) => {
        const nonExecutable = join(baseDir, 'nope');
        await writeFile(nonExecutable, '#!/bin/sh\nexit 0\n', 'utf8');
        await chmod(nonExecutable, 0o644);

        await assert.rejects(
            executeCommand(nonExecutable),
            /Failed to execute command/
        );
    });
});

test('executeHtmlcut and processWithHtmlcut work with the new extract schema', async () => {
    await withTempDir('ffhn-process-', async (baseDir) => {
        const { binDir } = await createFakeHtmlcutBin(baseDir, 'success');
        await withPatchedPath(binDir, async () => {
            const result = await executeHtmlcut(' abc ', {
                from: '<main>',
                to: '</main>',
                pattern: 'regex',
                flags: 'u',
                capture: 'outer',
                all: true
            }, {
                timeoutMs: 5000,
                baseUrl: 'https://example.com/page'
            });

            assert.equal(result.exitCode, 0);
            assert.equal(result.stdout, 'ABC\n');

            const processed = await processWithHtmlcut(' xyz ', {
                from: '<main>',
                to: '</main>',
                pattern: 'literal',
                capture: 'inner',
                all: false
            }, {
                targetName: 'demo',
                timeoutMs: 5000,
                baseUrl: 'https://example.com/page'
            });
            assert.equal(processed, 'XYZ');
        });
    });
});

test('executeHtmlcut and processWithHtmlcut surface missing, failed, empty, and invalid inputs', async () => {
    await withTempDir('ffhn-process-', async (baseDir) => {
        await withExactPath(baseDir, async () => {
            await assert.rejects(
                executeHtmlcut('<main>x</main>', {
                    from: '<main>',
                    to: '</main>',
                    pattern: 'literal',
                    capture: 'inner',
                    all: false
                }),
                /HTMLCut binary not found/
            );
        });

        const fail = await createFakeHtmlcutBin(baseDir, 'fail');
        await withPatchedPath(fail.binDir, async () => {
            await assert.rejects(
                processWithHtmlcut('abc', {
                    from: 'a',
                    to: 'b',
                    pattern: 'literal',
                    capture: 'inner',
                    all: false
                }),
                /HTMLCut execution failed/
            );
        });

        const empty = await createFakeHtmlcutBin(baseDir, 'empty');
        await withPatchedPath(empty.binDir, async () => {
            await assert.rejects(
                processWithHtmlcut('abc', {
                    from: 'a',
                    to: 'b',
                    pattern: 'literal',
                    capture: 'inner',
                    all: false
                }),
                /empty output/
            );
        });

        const raw = await createFakeHtmlcutBin(baseDir, 'raw');
        await withPatchedPath(raw.binDir, async () => {
            const processed = await processWithHtmlcut('raw', {
                from: 'a',
                to: 'b',
                pattern: 'literal',
                capture: 'inner',
                all: false
            });
            assert.equal(processed, 'RAW');
        });

        const crlf = await createFakeHtmlcutBin(baseDir, 'crlf');
        await withPatchedPath(crlf.binDir, async () => {
            const processed = await processWithHtmlcut('crlf', {
                from: 'a',
                to: 'b',
                pattern: 'literal',
                capture: 'inner',
                all: false
            });
            assert.equal(processed, 'CRLF');
        });

        await assert.rejects(
            processWithHtmlcut(null, {
                from: 'a',
                to: 'b',
                pattern: 'literal',
                capture: 'inner',
                all: false
            }),
            /HTML input must be a string/
        );
    });
});
