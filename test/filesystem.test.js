import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import {
    ensureDirectoryExists,
    pathExists,
    readTextFile,
    readTextFileIfExists,
    writeTextFileAtomic,
    writeJsonFileAtomic,
    removeFileIfExists
} from '../lib/adapters/filesystem.js';
import { readJsonFile, withTempDir } from './helpers.js';

test('ensureDirectoryExists and pathExists handle directories and files', async () => {
    await withTempDir('ffhn-fs-', async (baseDir) => {
        const dir = join(baseDir, 'a', 'b');
        await ensureDirectoryExists(dir);
        assert.equal(await pathExists(dir), true);
        assert.equal(await pathExists(join(baseDir, 'missing')), false);
    });
});

test('readTextFile reads files and wraps filesystem failures', async () => {
    await withTempDir('ffhn-fs-', async (baseDir) => {
        const filePath = join(baseDir, 'demo.txt');
        await writeFile(filePath, 'hello', 'utf8');
        assert.equal(await readTextFile(filePath), 'hello');
        assert.equal(await readTextFileIfExists(join(baseDir, 'missing.txt')), null);

        await assert.rejects(
            readTextFile(baseDir),
            /Failed to read file/
        );
    });
});

test('writeTextFileAtomic and writeJsonFileAtomic create parent directories', async () => {
    await withTempDir('ffhn-fs-', async (baseDir) => {
        const textPath = join(baseDir, 'nested', 'file.txt');
        const jsonPath = join(baseDir, 'nested', 'state.json');

        await writeTextFileAtomic(textPath, 'hello');
        await writeJsonFileAtomic(jsonPath, { ok: true });

        assert.equal(await readTextFile(textPath), 'hello');
        assert.deepEqual(await readJsonFile(jsonPath), { ok: true });

        await assert.rejects(
            writeTextFileAtomic(textPath, 123),
            /File content must be a string/
        );
    });
});

test('writeTextFileAtomic and removeFileIfExists wrap edge-case failures', async () => {
    await withTempDir('ffhn-fs-', async (baseDir) => {
        const blocker = join(baseDir, 'blocker');
        await writeFile(blocker, 'x', 'utf8');

        await assert.rejects(
            writeTextFileAtomic(join(blocker, 'child.txt'), 'hello'),
            /Failed to write file/
        );

        const removable = join(baseDir, 'remove-me.txt');
        await writeFile(removable, 'bye', 'utf8');
        await removeFileIfExists(removable);
        assert.equal(await pathExists(removable), false);

        await removeFileIfExists(join(baseDir, 'already-gone.txt'));

        await assert.rejects(
            removeFileIfExists('/dev/null/child'),
            /Failed to delete file/
        );
    });
});

test('readTextFileIfExists wraps non-ENOENT read failures', async () => {
    await withTempDir('ffhn-fs-', async (baseDir) => {
        const dir = join(baseDir, 'dir');
        await mkdir(dir);

        await assert.rejects(
            readTextFileIfExists(dir),
            /Failed to read file/
        );
    });
});
