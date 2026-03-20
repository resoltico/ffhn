import { join, resolve, dirname } from 'node:path';
import { readdir } from 'node:fs/promises';
import { FILES, DEFAULTS } from '../core/constants.js';
import { pathExists } from '../adapters/filesystem.js';

export function resolveWatchlistDir(watchlistDir = DEFAULTS.WATCHLIST_DIR) {
    return resolve(watchlistDir);
}

export function buildTargetPaths(targetDir) {
    const absoluteTargetDir = resolve(targetDir);
    return {
        dir: absoluteTargetDir,
        config: join(absoluteTargetDir, FILES.CONFIG),
        state: join(absoluteTargetDir, FILES.STATE),
        current: join(absoluteTargetDir, FILES.CURRENT),
        previous: join(absoluteTargetDir, FILES.PREVIOUS)
    };
}

export async function* discoverTargets(watchlistBase = DEFAULTS.WATCHLIST_DIR) {
    const baseDir = resolveWatchlistDir(watchlistBase);
    let entries = [];
    try {
        entries = await readdir(baseDir, { withFileTypes: true });
    } catch (error) {
        if (error.code === 'ENOENT') {
            return;
        }
        throw error;
    }

    entries.sort((a, b) => a.name.localeCompare(b.name));

    for (const entry of entries) {
        if (!entry.isDirectory() || entry.name.startsWith('.')) {
            continue;
        }

        const targetDir = join(baseDir, entry.name);
        const configPath = join(targetDir, FILES.CONFIG);

        if (await pathExists(configPath)) {
            yield {
                name: entry.name,
                dir: targetDir,
                configPath
            };
        }
    }
}

export function getParentDir(filePath) {
    return dirname(filePath);
}
