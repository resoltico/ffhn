import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

import { buildTargetPaths, discoverTargets, getParentDir, resolveWatchlistDir } from '../lib/utils/path.js';
import { withTempDir } from './helpers.js';

test('buildTargetPaths returns the new state-aware file layout', () => {
    const paths = buildTargetPaths('/tmp/demo');
    assert.equal(paths.config, '/tmp/demo/target.toml');
    assert.equal(paths.state, '/tmp/demo/state.json');
    assert.equal(paths.current, '/tmp/demo/current.txt');
    assert.equal(paths.previous, '/tmp/demo/previous.txt');
});

test('resolveWatchlistDir and getParentDir normalize paths', () => {
    assert.equal(resolveWatchlistDir('./watchlist'), resolve('./watchlist'));
    assert.equal(getParentDir('/a/b/c.txt'), '/a/b');
});

test('discoverTargets yields sorted non-hidden directories with target configs', async () => {
    await withTempDir('ffhn-path-', async (baseDir) => {
        const watchlist = join(baseDir, 'watchlist');
        const validOne = join(watchlist, 'aaa');
        const validTwo = join(watchlist, 'zzz');
        const hidden = join(watchlist, '.hidden');
        const invalid = join(watchlist, 'invalid');

        await mkdir(validOne, { recursive: true });
        await mkdir(validTwo, { recursive: true });
        await mkdir(hidden, { recursive: true });
        await mkdir(invalid, { recursive: true });

        const configBody = '[target]\nurl="https://example.com"\n[extract]\nstart="a"\nend="b"\n';
        await writeFile(join(validOne, 'target.toml'), configBody, 'utf8');
        await writeFile(join(validTwo, 'target.toml'), configBody, 'utf8');
        await writeFile(join(hidden, 'target.toml'), configBody, 'utf8');

        const found = [];
        for await (const target of discoverTargets(watchlist)) {
            found.push(target);
        }

        assert.deepEqual(found.map((target) => target.name), ['aaa', 'zzz']);
        assert.equal(found[0].configPath, join(validOne, 'target.toml'));
    });
});

test('discoverTargets returns empty iterator for missing watchlists and rethrows other errors', async () => {
    await withTempDir('ffhn-path-', async (baseDir) => {
        const missing = join(baseDir, 'missing');
        const found = [];

        for await (const target of discoverTargets(missing)) {
            found.push(target);
        }

        assert.deepEqual(found, []);

        const notDirectory = join(baseDir, 'file');
        await writeFile(notDirectory, 'x', 'utf8');
        const iterator = discoverTargets(notDirectory);
        await assert.rejects(iterator.next(), /ENOTDIR/);
    });
});
