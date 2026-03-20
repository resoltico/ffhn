import test from 'node:test';
import assert from 'node:assert/strict';
import pkg from '../package.json' with { type: 'json' };

import { main, normalizeCliParseError } from '../lib/cli.js';
import { EXIT_CODES } from '../lib/core/constants.js';

function captureConsole() {
    const logs = [];
    const errors = [];
    const originalStdoutWrite = process.stdout.write;
    const originalStderrWrite = process.stderr.write;

    process.stdout.write = (chunk) => {
        logs.push(String(chunk));
        return true;
    };
    process.stderr.write = (chunk) => {
        errors.push(String(chunk));
        return true;
    };

    return {
        logs,
        errors,
        restore() {
            process.stdout.write = originalStdoutWrite;
            process.stderr.write = originalStderrWrite;
        }
    };
}

test('main emits version payload and exits successfully', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['--version'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.equal(payload.command, 'version');
    assert.equal(payload.ffhn_version, pkg.version);
});

test('main supports pretty json for version and help output', async () => {
    let versionExitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['--version', '--pretty'], {
            exitFn(code) {
                versionExitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(versionExitCode, EXIT_CODES.SUCCESS);
    assert.match(versionCapture.logs.join(''), /\n  "command": "version"/);

    let helpExitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['--help', '--pretty'], {
            exitFn(code) {
                helpExitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(helpExitCode, EXIT_CODES.SUCCESS);
    assert.match(helpCapture.logs.join(''), /\n  "command": "help"/);
});

test('main emits help payload', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['--help'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.equal(payload.command, 'help');
    assert.match(payload.usage, /ffhn <command> \[options\]/);
    assert.equal(payload.commands.at(-2).name, 'help');
    assert.equal(payload.commands.at(-1).name, 'version');
    assert.equal(payload.topic, undefined);
});

test('main writes usage errors to stderr as json', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['wat'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNKNOWN_COMMAND');
    assert.equal(payload.error.type, 'usage');
});

test('main maps unknown options to usage errors', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['--wat'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNKNOWN_OPTION');
    assert.equal(payload.error.type, 'usage');
    assert.equal(payload.error.details.parse_error_code, 'ERR_PARSE_ARGS_UNKNOWN_OPTION');
});

test('main supports positional help and version commands', async () => {
    let helpExitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['help'], {
            exitFn(code) {
                helpExitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(helpExitCode, EXIT_CODES.SUCCESS);
    assert.equal(JSON.parse(helpCapture.logs.join('').trim()).command, 'help');

    let versionExitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['version'], {
            exitFn(code) {
                versionExitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(versionExitCode, EXIT_CODES.SUCCESS);
    assert.equal(JSON.parse(versionCapture.logs.join('').trim()).command, 'version');
});

test('main emits scoped help payloads for command topics', async () => {
    let helpExitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['help', 'run'], {
            exitFn(code) {
                helpExitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(helpExitCode, EXIT_CODES.SUCCESS);
    const helpPayload = JSON.parse(helpCapture.logs.join('').trim());
    assert.equal(helpPayload.command, 'help');
    assert.equal(helpPayload.topic, 'run');
    assert.equal(helpPayload.usage, 'ffhn run [options]');
    assert.ok(helpPayload.options.some((option) => option.flag === '--concurrency'));

    let statusExitCode = null;
    const statusCapture = captureConsole();

    try {
        await main(['status', '--help'], {
            exitFn(code) {
                statusExitCode = code;
            }
        });
    } finally {
        statusCapture.restore();
    }

    assert.equal(statusExitCode, EXIT_CODES.SUCCESS);
    const statusPayload = JSON.parse(statusCapture.logs.join('').trim());
    assert.equal(statusPayload.topic, 'status');
    assert.equal(statusPayload.usage, 'ffhn status [options]');

    let versionHelpExitCode = null;
    const versionHelpCapture = captureConsole();

    try {
        await main(['--help', 'version'], {
            exitFn(code) {
                versionHelpExitCode = code;
            }
        });
    } finally {
        versionHelpCapture.restore();
    }

    assert.equal(versionHelpExitCode, EXIT_CODES.SUCCESS);
    const versionHelpPayload = JSON.parse(versionHelpCapture.logs.join('').trim());
    assert.equal(versionHelpPayload.topic, 'version');
    assert.equal(versionHelpPayload.usage, 'ffhn version [options]');
});

test('main accepts redundant matching meta commands and rejects extra trailing arguments', async () => {
    let helpExitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['--help', 'help'], {
            exitFn(code) {
                helpExitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(helpExitCode, EXIT_CODES.SUCCESS);
    const helpPayload = JSON.parse(helpCapture.logs.join('').trim());
    assert.equal(helpPayload.command, 'help');
    assert.equal(helpPayload.topic, undefined);

    let versionExitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['--version', 'version'], {
            exitFn(code) {
                versionExitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(versionExitCode, EXIT_CODES.SUCCESS);
    assert.equal(JSON.parse(versionCapture.logs.join('').trim()).command, 'version');

    let extraExitCode = null;
    const extraCapture = captureConsole();

    try {
        await main(['--help', 'help', 'extra'], {
            exitFn(code) {
                extraExitCode = code;
            }
        });
    } finally {
        extraCapture.restore();
    }

    assert.equal(extraExitCode, EXIT_CODES.USAGE_ERROR);
    const extraPayload = JSON.parse(extraCapture.errors.join('').trim());
    assert.equal(extraPayload.error.code, 'UNKNOWN_COMMAND');
});

test('main accepts help/version flags alongside operational commands', async () => {
    let helpExitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['run', '--help', '--watchlist', '/tmp/demo-watchlist'], {
            exitFn(code) {
                helpExitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(helpExitCode, EXIT_CODES.SUCCESS);
    const helpPayload = JSON.parse(helpCapture.logs.join('').trim());
    assert.equal(helpPayload.command, 'help');
    assert.equal(helpPayload.topic, 'run');

    let versionExitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['--version', 'status', '--target', 'demo'], {
            exitFn(code) {
                versionExitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(versionExitCode, EXIT_CODES.SUCCESS);
    assert.equal(JSON.parse(versionCapture.logs.join('').trim()).command, 'version');

    let unsupportedExitCode = null;
    const unsupportedCapture = captureConsole();

    try {
        await main(['init', '--help', '--concurrency', '2'], {
            exitFn(code) {
                unsupportedExitCode = code;
            }
        });
    } finally {
        unsupportedCapture.restore();
    }

    assert.equal(unsupportedExitCode, EXIT_CODES.USAGE_ERROR);
    const unsupportedPayload = JSON.parse(unsupportedCapture.errors.join('').trim());
    assert.equal(unsupportedPayload.error.code, 'UNSUPPORTED_OPTION');
});

test('main rejects unknown help topics as unknown commands', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['help', 'wat'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNKNOWN_COMMAND');
});

test('main rejects extra arguments after scoped help topics and unknown commands after version flags', async () => {
    let exitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['help', 'run', 'extra'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    let payload = JSON.parse(helpCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNEXPECTED_ARGUMENTS');

    exitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['--version', 'wat'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    payload = JSON.parse(versionCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNKNOWN_COMMAND');
});

test('main rejects extra arguments after meta-flag command subjects', async () => {
    let exitCode = null;
    const helpCapture = captureConsole();

    try {
        await main(['--help', 'run', 'extra'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        helpCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    let payload = JSON.parse(helpCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNEXPECTED_ARGUMENTS');

    exitCode = null;
    const versionCapture = captureConsole();

    try {
        await main(['--version', 'status', 'extra'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        versionCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    payload = JSON.parse(versionCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNEXPECTED_ARGUMENTS');
});

test('main maps missing option values to usage errors', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['run', '--target'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'INVALID_ARGUMENTS');
    assert.equal(payload.error.type, 'usage');
    assert.equal(payload.error.details.parse_error_code, 'ERR_PARSE_ARGS_INVALID_OPTION_VALUE');
});

test('main rejects conflicting commands, unsupported options, and extra positionals', async () => {
    let exitCode = null;
    const conflictingCapture = captureConsole();

    try {
        await main(['--help', '--version'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        conflictingCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    let payload = JSON.parse(conflictingCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'CONFLICTING_COMMANDS');

    exitCode = null;
    const positionalConflictCapture = captureConsole();

    try {
        await main(['help', '--version'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        positionalConflictCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    payload = JSON.parse(positionalConflictCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'CONFLICTING_COMMANDS');

    exitCode = null;
    const unsupportedCapture = captureConsole();

    try {
        await main(['status', '--concurrency', '2'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        unsupportedCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    payload = JSON.parse(unsupportedCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNSUPPORTED_OPTION');

    exitCode = null;
    const positionalCapture = captureConsole();

    try {
        await main(['run', 'extra'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        positionalCapture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    payload = JSON.parse(positionalCapture.errors.join('').trim());
    assert.equal(payload.error.code, 'UNEXPECTED_ARGUMENTS');
});

test('main surfaces invalid concurrency as a usage error', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['run', '--concurrency', '4.5'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'INVALID_CONCURRENCY');
});

test('main treats zero concurrency as a usage error', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['run', '--concurrency', '0'], {
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.USAGE_ERROR);
    const payload = JSON.parse(capture.errors.join('').trim());
    assert.equal(payload.error.code, 'INVALID_CONCURRENCY');
});

test('main pretty prints command payloads through injected runtime', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['status', '--pretty'], {
            discoverTargetsFn: async function* discoverTargetsFn() {},
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    assert.match(capture.logs.join(''), /\n  "command": "status"/);
});

test('main accepts explicit json output requests', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['status', '--json'], {
            discoverTargetsFn: async function* discoverTargetsFn() {},
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.equal(payload.command, 'status');
});

test('normalizeCliParseError preserves non-parse errors', () => {
    const error = new Error('boom');
    assert.equal(normalizeCliParseError(error), error);
});

test('main preserves an explicit watchlist option during argument parsing', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['status', '--watchlist', '/tmp/custom-watchlist'], {
            discoverTargetsFn: async function* discoverTargetsFn() {},
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.match(payload.watchlist, /\/tmp\/custom-watchlist$/);
});

test('main converts unexpected runtime failures into structured json errors', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main(['run'], {
            discoverTargetsFn: async function* discoverTargetsFn() {
                yield {
                    name: 'demo',
                    dir: '/tmp/demo',
                    configPath: '/tmp/demo/target.toml'
                };
            },
            async loadTargetFn() {
                throw new Error('boom');
            },
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.CONFIG_ERROR);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.equal(payload.command, 'run');
    assert.equal(payload.summary.failed, 1);
});

test('main defaults to run when no command is provided', async () => {
    let exitCode = null;
    const capture = captureConsole();

    try {
        await main([], {
            discoverTargetsFn: async function* discoverTargetsFn() {
                yield {
                    name: 'demo',
                    dir: '/tmp/demo',
                    configPath: '/tmp/demo/target.toml'
                };
            },
            async loadTargetFn(discoveredTarget) {
                return {
                    name: discoveredTarget.name,
                    url: 'https://example.com',
                    paths: {
                        dir: discoveredTarget.dir,
                        config: discoveredTarget.configPath,
                        state: `${discoveredTarget.dir}/state.json`,
                        current: `${discoveredTarget.dir}/current.txt`,
                        previous: `${discoveredTarget.dir}/previous.txt`
                    }
                };
            },
            async processMultipleTargetsFn() {
                return [
                    {
                        name: 'demo',
                        status: 'initialized',
                        timing: { total_ms: 1 }
                    }
                ];
            },
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    const payload = JSON.parse(capture.logs.join('').trim());
    assert.equal(payload.command, 'run');
});

test('main accepts a valid integer concurrency value', async () => {
    let exitCode = null;
    let seenConcurrency = null;
    const capture = captureConsole();

    try {
        await main(['run', '--concurrency', '4'], {
            discoverTargetsFn: async function* discoverTargetsFn() {
                yield {
                    name: 'demo',
                    dir: '/tmp/demo',
                    configPath: '/tmp/demo/target.toml'
                };
            },
            async loadTargetFn(discoveredTarget) {
                return {
                    name: discoveredTarget.name,
                    url: 'https://example.com',
                    paths: {
                        dir: discoveredTarget.dir,
                        config: discoveredTarget.configPath,
                        state: `${discoveredTarget.dir}/state.json`,
                        current: `${discoveredTarget.dir}/current.txt`,
                        previous: `${discoveredTarget.dir}/previous.txt`
                    }
                };
            },
            async processMultipleTargetsFn(targets, concurrency) {
                seenConcurrency = concurrency;
                return targets.map((target) => ({
                    name: target.name,
                    status: 'initialized',
                    timing: { total_ms: 1 }
                }));
            },
            exitFn(code) {
                exitCode = code;
            }
        });
    } finally {
        capture.restore();
    }

    assert.equal(exitCode, EXIT_CODES.SUCCESS);
    assert.equal(seenConcurrency, 4);
});
