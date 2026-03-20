import { mkdtemp, mkdir, writeFile, chmod, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import http from 'node:http';

export async function withTempDir(prefix, fn) {
    const dir = await mkdtemp(join(tmpdir(), prefix));
    try {
        return await fn(dir);
    } finally {
        await rm(dir, { recursive: true, force: true });
    }
}

export async function createFakeHtmlcutBin(baseDir, mode = 'success') {
    const binDir = join(baseDir, 'bin');
    await mkdir(binDir, { recursive: true });
    const scriptPath = join(binDir, 'htmlcut');

    const script = `#!/usr/bin/env node
import process from 'node:process';

const args = process.argv.slice(2);
const inputSource = args[0];
const fromIndex = args.indexOf('--from');
const toIndex = args.indexOf('--to');
const patternIndex = args.indexOf('--pattern');
const formatIndex = args.indexOf('--format');
const captureIndex = args.indexOf('--capture');
const flagsIndex = args.indexOf('--flags');
const baseUrlIndex = args.indexOf('--base-url');

for (const unsupported of ['--start-pattern', '--end-pattern', '--output', '--quiet']) {
    if (args.includes(unsupported)) {
        console.error('unsupported legacy flag: ' + unsupported);
        process.exit(2);
    }
}

if (inputSource !== '-') {
    console.error('expected stdin input');
    process.exit(2);
}

if (fromIndex < 0 || toIndex < 0 || patternIndex < 0) {
    console.error('missing delimiter flags');
    process.exit(2);
}

const pattern = args[patternIndex + 1];
if (!['literal', 'regex'].includes(pattern)) {
    console.error('expected --pattern literal|regex');
    process.exit(2);
}

if (formatIndex < 0 || args[formatIndex + 1] !== 'text') {
    console.error('expected --format text');
    process.exit(2);
}

if (captureIndex >= 0 && !['inner', 'outer'].includes(args[captureIndex + 1])) {
    console.error('expected --capture inner|outer');
    process.exit(2);
}

if (flagsIndex >= 0 && pattern !== 'regex') {
    console.error('expected --flags only with --pattern regex');
    process.exit(2);
}

if (baseUrlIndex >= 0 && !/^https?:\\/\\//.test(args[baseUrlIndex + 1] || '')) {
    console.error('invalid --base-url');
    process.exit(2);
}

const mode = ${JSON.stringify(mode)};
if (mode === 'fail') {
    console.error('fake htmlcut failure');
    process.exit(1);
}

let input = '';
for await (const chunk of process.stdin) {
    input += chunk.toString();
}

if (mode === 'empty') {
    process.stdout.write('\\n');
    process.exit(0);
}

if (mode === 'extract') {
    const from = args[fromIndex + 1];
    const to = args[toIndex + 1];
    const capture = captureIndex >= 0 ? args[captureIndex + 1] : 'inner';
    const flags = flagsIndex >= 0 ? args[flagsIndex + 1] : '';

    function findMatch(text, value) {
        if (pattern === 'regex') {
            const match = new RegExp(value, flags).exec(text);
            if (!match) {
                return null;
            }

            return {
                index: match.index,
                matchText: match[0]
            };
        }

        const index = text.indexOf(value);
        if (index < 0) {
            return null;
        }

        return {
            index,
            matchText: value
        };
    }

    const fromMatch = findMatch(input, from);
    if (!fromMatch) {
        process.stdout.write('\\n');
        process.exit(0);
    }

    const searchStart = fromMatch.index + fromMatch.matchText.length;
    const remainder = input.slice(searchStart);
    const toMatch = findMatch(remainder, to);
    if (!toMatch) {
        process.stdout.write('\\n');
        process.exit(0);
    }

    const start = capture === 'outer'
        ? fromMatch.index
        : searchStart;
    const end = capture === 'outer'
        ? searchStart + toMatch.index + toMatch.matchText.length
        : searchStart + toMatch.index;

    process.stdout.write(input.slice(start, end) + '\\n');
    process.exit(0);
}

if (mode === 'raw') {
    process.stdout.write(input.toUpperCase().trim());
    process.exit(0);
}

if (mode === 'crlf') {
    process.stdout.write(input.toUpperCase().trim() + '\\r\\n');
    process.exit(0);
}

process.stdout.write(input.toUpperCase().trim() + '\\n');
`;

    await writeFile(scriptPath, script, 'utf8');
    await chmod(scriptPath, 0o755);
    return { binDir, scriptPath };
}

export async function withPatchedPath(binDir, fn) {
    const originalPath = process.env.PATH || '';
    process.env.PATH = `${binDir}:${originalPath}`;
    try {
        return await fn();
    } finally {
        process.env.PATH = originalPath;
    }
}

export async function withExactPath(pathValue, fn) {
    const originalPath = process.env.PATH || '';
    process.env.PATH = pathValue;
    try {
        return await fn();
    } finally {
        process.env.PATH = originalPath;
    }
}

export async function readJsonFile(filePath) {
    return JSON.parse(await readFile(filePath, 'utf8'));
}

export async function createServer(handler) {
    const server = http.createServer(handler);
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const address = server.address();
    const baseUrl = `http://127.0.0.1:${address.port}`;
    return {
        baseUrl,
        close: () => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()))
    };
}
