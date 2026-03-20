import { readFile, writeFile, mkdir, access, rename, unlink } from 'node:fs/promises';
import { constants } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { getParentDir } from '../utils/path.js';
import { FORMAT, ERROR_TYPES } from '../core/constants.js';
import { targetFailure } from '../utils/error.js';

export async function ensureDirectoryExists(dirPath) {
    await mkdir(dirPath, { recursive: true });
}

export async function pathExists(filePath) {
    try {
        await access(filePath, constants.F_OK);
        return true;
    } catch {
        return false;
    }
}

export async function readTextFile(filePath) {
    try {
        return await readFile(filePath, 'utf8');
    } catch (error) {
        throw targetFailure(
            'FILE_READ_FAILED',
            ERROR_TYPES.FILESYSTEM,
            `Failed to read file ${filePath}: ${error.message}`,
            {
                path: filePath
            },
            error
        );
    }
}

export async function readTextFileIfExists(filePath) {
    try {
        return await readFile(filePath, 'utf8');
    } catch (error) {
        if (error.code === 'ENOENT') {
            return null;
        }

        throw targetFailure(
            'FILE_READ_FAILED',
            ERROR_TYPES.FILESYSTEM,
            `Failed to read file ${filePath}: ${error.message}`,
            {
                path: filePath
            },
            error
        );
    }
}

export async function writeTextFileAtomic(filePath, content) {
    if (typeof content !== 'string') {
        throw new TypeError('File content must be a string');
    }

    try {
        await ensureDirectoryExists(getParentDir(filePath));
        const tempPath = `${filePath}.${randomUUID()}.tmp`;
        await writeFile(tempPath, content, 'utf8');
        await rename(tempPath, filePath);
    } catch (error) {
        throw targetFailure(
            'FILE_WRITE_FAILED',
            ERROR_TYPES.FILESYSTEM,
            `Failed to write file ${filePath}: ${error.message}`,
            {
                path: filePath
            },
            error
        );
    }
}

export async function writeJsonFileAtomic(filePath, value) {
    await writeTextFileAtomic(filePath, JSON.stringify(value, null, FORMAT.JSON_INDENT));
}

export async function removeFileIfExists(filePath) {
    try {
        await unlink(filePath);
    } catch (error) {
        if (error.code !== 'ENOENT') {
            throw targetFailure('FILE_DELETE_FAILED', ERROR_TYPES.FILESYSTEM, `Failed to delete file: ${filePath}`, {
                path: filePath
            }, error);
        }
    }
}
