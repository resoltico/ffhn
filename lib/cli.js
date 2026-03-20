import { exit } from 'node:process';
import { parseArgs } from 'node:util';
import { COMMANDS, CLI, DEFAULTS, ERROR_TYPES, EXIT_CODES, STATUS, SCHEMA_VERSION } from './core/constants.js';
import { VERSION } from './core/version.js';
import { createTargetTemplate, loadTarget } from './core/target.js';
import { buildChangeDetails, buildFailureContentDetails, buildFiles, buildRunPayload, createTargetLoadFailure, processMultipleTargets } from './core/pipeline.js';
import { discoverTargets, resolveWatchlistDir } from './utils/path.js';
import { ensureDirectoryExists, pathExists, readTextFileIfExists, writeTextFileAtomic } from './adapters/filesystem.js';
import { computeHash } from './services/hasher.js';
import { readState } from './services/state.js';
import { configError, normalizeError, serializeError, targetFailure, usageError } from './utils/error.js';
import { validateTargetName } from './utils/validation.js';

const DEFAULT_RUNTIME = {
    exitFn: exit,
    discoverTargetsFn: discoverTargets,
    loadTargetFn: loadTarget,
    processMultipleTargetsFn: processMultipleTargets,
    nowFn: Date.now,
    readStateFn: readState,
    pathExistsFn: pathExists,
    readTextFileIfExistsFn: readTextFileIfExists
};

const COMMAND_OPTION_SUPPORT = {
    [COMMANDS.RUN]: new Set(['target', 'watchlist', 'concurrency', 'json', 'pretty']),
    [COMMANDS.INIT]: new Set(['target', 'watchlist', 'json', 'pretty']),
    [COMMANDS.STATUS]: new Set(['target', 'watchlist', 'json', 'pretty']),
    [COMMANDS.HELP]: new Set(['json', 'pretty']),
    [COMMANDS.VERSION]: new Set(['json', 'pretty'])
};

const OPTION_HELP = {
    target: { flag: '--target', alias: '-t', description: 'Target name for init/run/status' },
    watchlist: { flag: '--watchlist', alias: '-w', description: 'Watchlist directory (default: watchlist)' },
    concurrency: { flag: '--concurrency', alias: '-c', description: 'Parallel target runs (run only)' },
    json: { flag: '--json', description: 'Emit compact JSON output (default)' },
    pretty: { flag: '--pretty', description: 'Pretty-print JSON output' },
    help: { flag: '--help', alias: '-h', description: 'Emit JSON help output (or use `ffhn help`)' },
    version: { flag: '--version', alias: '-V', description: 'Emit JSON version output (or use `ffhn version`)' }
};

const COMMAND_HELP = {
    [COMMANDS.RUN]: {
        usage: 'ffhn run [options]',
        description: 'Process targets and emit a JSON execution report',
        options: ['target', 'watchlist', 'concurrency', 'json', 'pretty', 'help', 'version'],
        examples: [
            'ffhn run',
            'ffhn run --target news-site --pretty',
            'ffhn run --watchlist ./watchlist --concurrency 4'
        ]
    },
    [COMMANDS.INIT]: {
        usage: 'ffhn init --target <name> [options]',
        description: 'Create a target directory with a fresh target.toml template',
        options: ['target', 'watchlist', 'json', 'pretty', 'help', 'version'],
        examples: [
            'ffhn init --target news-site',
            'ffhn init --target news-site --watchlist ./watchlist'
        ]
    },
    [COMMANDS.STATUS]: {
        usage: 'ffhn status [options]',
        description: 'Inspect configured targets and current state files',
        options: ['target', 'watchlist', 'json', 'pretty', 'help', 'version'],
        examples: [
            'ffhn status',
            'ffhn status --target news-site',
            'ffhn status --watchlist ./watchlist --pretty'
        ]
    },
    [COMMANDS.HELP]: {
        usage: 'ffhn help [command] [options]',
        description: 'Emit JSON help output',
        options: ['json', 'pretty', 'help', 'version'],
        examples: [
            'ffhn help',
            'ffhn help run',
            'ffhn --help status'
        ]
    },
    [COMMANDS.VERSION]: {
        usage: 'ffhn version [options]',
        description: 'Emit JSON version output',
        options: ['json', 'pretty', 'help', 'version'],
        examples: [
            'ffhn version',
            'ffhn --version',
            'ffhn --help version'
        ]
    }
};

export async function main(argv = process.argv.slice(CLI.ARGV_OFFSET), runtime = {}) {
    const mergedRuntime = {
        ...DEFAULT_RUNTIME,
        ...runtime
    };

    try {
        const { command, helpTopic, options } = parseCliArgs(argv);

        if (command === COMMANDS.VERSION) {
            writeJson({
                command: COMMANDS.VERSION,
                exit_code: EXIT_CODES.SUCCESS,
                ffhn_version: VERSION,
                schema_version: SCHEMA_VERSION
            }, options.pretty);
            mergedRuntime.exitFn(EXIT_CODES.SUCCESS);
            return;
        }

        if (command === COMMANDS.HELP) {
            writeJson(buildHelpPayload(helpTopic), options.pretty);
            mergedRuntime.exitFn(EXIT_CODES.SUCCESS);
            return;
        }

        const response = await dispatchCommand(command, options, mergedRuntime);
        writeJson(response, options.pretty);
        mergedRuntime.exitFn(response.exit_code);
    } catch (error) {
        const normalizedError = normalizeError(error);
        writeJson({
            command: null,
            exit_code: normalizedError.exitCode || EXIT_CODES.INTERNAL_ERROR,
            error: serializeError(normalizedError)
        }, true, process.stderr);
        mergedRuntime.exitFn(normalizedError.exitCode || EXIT_CODES.INTERNAL_ERROR);
    }
}

export async function runCommand(options, runtime = DEFAULT_RUNTIME) {
    const resolvedRuntime = {
        ...DEFAULT_RUNTIME,
        ...runtime
    };
    const startedAt = resolvedRuntime.nowFn();
    const watchlistPath = resolveWatchlistDir(options.watchlist);
    const concurrency = options.concurrency === undefined
        ? DEFAULTS.CONCURRENCY
        : normalizeConcurrencyOption(options.concurrency);

    if (options.target) {
        validateTargetOption(options.target);
    }

    const targetSlots = [];

    for await (const discoveredTarget of resolvedRuntime.discoverTargetsFn(watchlistPath)) {
        if (options.target && discoveredTarget.name !== options.target) {
            continue;
        }

        try {
            targetSlots.push({
                type: 'ready',
                discoveredTarget,
                target: await resolvedRuntime.loadTargetFn(discoveredTarget)
            });
        } catch (error) {
            targetSlots.push({
                type: 'failed',
                discoveredTarget,
                result: createTargetLoadFailure(discoveredTarget, error)
            });
        }
    }

    if (targetSlots.length === 0) {
        throw configError(
            options.target ? 'TARGET_NOT_FOUND' : 'NO_TARGETS',
            options.target
                ? `Target not found: ${options.target}`
                : `No targets found in ${watchlistPath}`
        );
    }

    const runnableTargets = targetSlots
        .filter((slot) => slot.type === 'ready')
        .map((slot) => slot.target);
    const processedTargets = await resolvedRuntime.processMultipleTargetsFn(runnableTargets, concurrency);

    let processedIndex = 0;
    const results = targetSlots.map((slot) => {
        if (slot.type === 'failed') {
            return slot.result;
        }

        const result = processedTargets[processedIndex];
        processedIndex++;
        return result;
    });

    const finishedAt = resolvedRuntime.nowFn();
    const payload = buildRunPayload(results, startedAt, finishedAt, watchlistPath, {
        target: options.target ?? null,
        concurrency
    });

    return {
        command: COMMANDS.RUN,
        exit_code: determineRunExitCode(results),
        ...payload
    };
}

export async function initCommand(options) {
    if (!options.target) {
        throw usageError('TARGET_REQUIRED', 'init requires --target');
    }

    validateTargetOption(options.target);

    const watchlistPath = resolveWatchlistDir(options.watchlist);
    const targetDir = `${watchlistPath}/${options.target}`;
    const configPath = `${targetDir}/${DEFAULTS.TARGET_CONFIG}`;

    if (await pathExists(targetDir)) {
        throw configError('TARGET_EXISTS', `Target already exists: ${targetDir}`);
    }

    await ensureDirectoryExists(targetDir);
    await writeTextFileAtomic(configPath, createTargetTemplate());

    return {
        command: COMMANDS.INIT,
        exit_code: EXIT_CODES.SUCCESS,
        ffhn_version: VERSION,
        schema_version: SCHEMA_VERSION,
        watchlist: watchlistPath,
        target: {
            name: options.target,
            directory: targetDir,
            config_path: configPath
        }
    };
}

export async function statusCommand(options, runtime = DEFAULT_RUNTIME) {
    const resolvedRuntime = {
        ...DEFAULT_RUNTIME,
        ...runtime
    };
    const watchlistPath = resolveWatchlistDir(options.watchlist);
    const targets = [];

    if (options.target) {
        validateTargetOption(options.target);
    }

    for await (const discoveredTarget of resolvedRuntime.discoverTargetsFn(watchlistPath)) {
        if (options.target && discoveredTarget.name !== options.target) {
            continue;
        }

        try {
            const target = await resolvedRuntime.loadTargetFn(discoveredTarget);
            const baseTarget = createStatusTargetBase(target.name, target.url, target.paths);
            const stateExists = await resolvedRuntime.pathExistsFn(target.paths.state);
            if (!stateExists) {
                const { content, artifacts } = await readStatusArtifacts(resolvedRuntime, null, target.paths);

                if (!artifacts.consistent) {
                    targets.push(createInvalidStatusTarget(baseTarget, createArtifactIntegrityError(target.name, artifacts), {
                        content,
                        change: null,
                        artifacts,
                        state: null
                    }));
                    continue;
                }

                targets.push({
                    ...baseTarget,
                    status: 'pending',
                    content,
                    change: null,
                    last_run: buildNeverRunDetails(),
                    artifacts,
                    state: null
                });
                continue;
            }

            let state;
            try {
                state = await resolvedRuntime.readStateFn(target.paths.state);
            } catch (error) {
                const normalizedError = normalizeError(error);
                targets.push(createInvalidStatusTarget(baseTarget, normalizedError));
                continue;
            }

            const { content, currentContent, previousContent, artifacts } = await readStatusArtifacts(resolvedRuntime, state, target.paths);
            const change = buildChangeDetails(currentContent, previousContent);

            if (!artifacts.consistent) {
                targets.push(createInvalidStatusTarget(baseTarget, createArtifactIntegrityError(target.name, artifacts), {
                    content,
                    change,
                    artifacts,
                    state
                }));
                continue;
            }

            targets.push({
                ...baseTarget,
                status: 'ready',
                content,
                change,
                last_run: buildLastRunDetails(state),
                artifacts,
                state
            });
        } catch (error) {
            const normalizedError = normalizeError(error);
            targets.push(createInvalidStatusTarget({
                name: discoveredTarget.name,
                url: null,
                files: {
                    directory: discoveredTarget.dir,
                    config: discoveredTarget.configPath,
                    state: `${discoveredTarget.dir}/${DEFAULTS.STATE_FILE}`,
                    current: `${discoveredTarget.dir}/${DEFAULTS.CURRENT_FILE}`,
                    previous: `${discoveredTarget.dir}/${DEFAULTS.PREVIOUS_FILE}`
                }
            }, normalizedError));
        }
    }

    if (targets.length === 0 && options.target) {
        throw configError('TARGET_NOT_FOUND', `Target not found: ${options.target}`);
    }

    const summary = summarizeStatusTargets(targets);

    return {
        command: COMMANDS.STATUS,
        exit_code: summary.invalid > 0 ? EXIT_CODES.CONFIG_ERROR : EXIT_CODES.SUCCESS,
        ffhn_version: VERSION,
        schema_version: SCHEMA_VERSION,
        watchlist: watchlistPath,
        selection: {
            target: options.target ?? null
        },
        count: targets.length,
        invalid_targets: summary.invalid,
        summary,
        targets
    };
}

function parseCliArgs(argv) {
    let parsedArgs;
    try {
        parsedArgs = parseArgs({
            args: argv,
            allowPositionals: true,
            options: {
                target: { type: 'string', short: 't' },
                watchlist: { type: 'string', short: 'w' },
                concurrency: { type: 'string', short: 'c' },
                json: { type: 'boolean' },
                pretty: { type: 'boolean' },
                help: { type: 'boolean', short: 'h' },
                version: { type: 'boolean', short: 'V' }
            }
        });
    } catch (error) {
        throw normalizeCliParseError(error);
    }

    const { values, positionals } = parsedArgs;

    const { command, optionScope, helpTopic } = resolveCommand(positionals, values);
    assertSupportedOptions(optionScope, values);

    const concurrency = values.concurrency === undefined
        ? undefined
        : parseConcurrency(values.concurrency);

    return {
        command,
        helpTopic,
        options: {
            target: values.target,
            watchlist: values.watchlist ?? DEFAULTS.WATCHLIST_DIR,
            concurrency,
            pretty: values.pretty ?? false
        }
    };
}

function dispatchCommand(command, options, runtime) {
    if (command === COMMANDS.RUN) {
        return runCommand(options, runtime);
    }

    if (command === COMMANDS.INIT) {
        return initCommand(options);
    }

    return statusCommand(options, runtime);
}

function buildHelpPayload(topic = null) {
    if (topic) {
        return buildCommandHelpPayload(topic);
    }

    return {
        command: COMMANDS.HELP,
        exit_code: EXIT_CODES.SUCCESS,
        ffhn_version: VERSION,
        schema_version: SCHEMA_VERSION,
        usage: 'ffhn <command> [options]',
        commands: [
            {
                name: COMMANDS.RUN,
                description: COMMAND_HELP[COMMANDS.RUN].description
            },
            {
                name: COMMANDS.INIT,
                description: COMMAND_HELP[COMMANDS.INIT].description
            },
            {
                name: COMMANDS.STATUS,
                description: COMMAND_HELP[COMMANDS.STATUS].description
            },
            {
                name: COMMANDS.HELP,
                description: COMMAND_HELP[COMMANDS.HELP].description
            },
            {
                name: COMMANDS.VERSION,
                description: COMMAND_HELP[COMMANDS.VERSION].description
            }
        ],
        options: [
            OPTION_HELP.target,
            OPTION_HELP.watchlist,
            OPTION_HELP.concurrency,
            OPTION_HELP.json,
            OPTION_HELP.pretty,
            OPTION_HELP.help,
            OPTION_HELP.version
        ],
        examples: [
            'ffhn init --target news-site',
            'ffhn run --target news-site --pretty',
            'ffhn status --target news-site --watchlist ./watchlist',
            'ffhn help',
            'ffhn help run',
            'ffhn version'
        ]
    };
}

function buildCommandHelpPayload(command) {
    const definition = COMMAND_HELP[command];
    return {
        command: COMMANDS.HELP,
        topic: command,
        exit_code: EXIT_CODES.SUCCESS,
        ffhn_version: VERSION,
        schema_version: SCHEMA_VERSION,
        usage: definition.usage,
        description: definition.description,
        options: definition.options.map((optionName) => OPTION_HELP[optionName]),
        examples: definition.examples
    };
}

function writeJson(payload, pretty, stream = process.stdout) {
    const body = pretty
        ? JSON.stringify(payload, null, 2)
        : JSON.stringify(payload);
    stream.write(`${body}\n`);
}

function determineRunExitCode(results) {
    const failedResults = results.filter((result) => result.status === STATUS.FAILED);

    if (failedResults.length === 0) {
        return EXIT_CODES.SUCCESS;
    }

    if (failedResults.some((result) => result.error?.type === 'internal')) {
        return EXIT_CODES.INTERNAL_ERROR;
    }

    if (failedResults.some((result) => result.error?.type === 'config')) {
        return EXIT_CODES.CONFIG_ERROR;
    }

    if (failedResults.some((result) => result.error?.type === 'dependency')) {
        return EXIT_CODES.DEPENDENCY_ERROR;
    }

    return EXIT_CODES.TARGET_FAILURE;
}

function parseConcurrency(rawValue) {
    if (!/^[0-9]+$/.test(rawValue)) {
        throw usageError('INVALID_CONCURRENCY', `Invalid concurrency: ${rawValue}`);
    }

    return normalizeConcurrencyOption(Number.parseInt(rawValue, 10), rawValue);
}

function validateTargetOption(targetName) {
    try {
        validateTargetName(targetName);
    } catch (error) {
        throw usageError('INVALID_TARGET', `Invalid --target value: ${error.message}`, null, error);
    }
}

export function normalizeCliParseError(error) {
    const normalizedError = normalizeError(error);
    const parseErrorCode = typeof normalizedError.code === 'string'
        ? normalizedError.code
        : null;

    if (!parseErrorCode?.startsWith('ERR_PARSE_ARGS_')) {
        return normalizedError;
    }

    const usageCode = parseErrorCode === 'ERR_PARSE_ARGS_UNKNOWN_OPTION'
        ? 'UNKNOWN_OPTION'
        : 'INVALID_ARGUMENTS';

    return usageError(usageCode, normalizedError.message, {
        parse_error_code: parseErrorCode
    }, normalizedError);
}

function resolveCommand(positionals, values) {
    const flagCommand = deriveFlagCommand(values);

    if (flagCommand) {
        return resolveFlagCommand(flagCommand, positionals);
    }

    const command = positionals[0] ?? COMMANDS.RUN;
    if (!isKnownCommand(command)) {
        throw usageError('UNKNOWN_COMMAND', `Unknown command: ${command}`);
    }

    if (command === COMMANDS.HELP) {
        return resolveHelpCommand(positionals.slice(1));
    }

    if (positionals.length > 1) {
        throw unexpectedArgumentsError(positionals.slice(1));
    }

    return {
        command,
        optionScope: command,
        helpTopic: null
    };
}

function resolveFlagCommand(flagCommand, positionals) {
    if (positionals.length === 0) {
        return {
            command: flagCommand,
            optionScope: flagCommand,
            helpTopic: null
        };
    }

    const [firstPositional, ...remainingPositionals] = positionals;
    if (firstPositional === flagCommand) {
        if (flagCommand === COMMANDS.HELP) {
            return resolveHelpCommand(remainingPositionals);
        }

        if (remainingPositionals.length > 0) {
            throw unexpectedArgumentsError(remainingPositionals);
        }

        return {
            command: flagCommand,
            optionScope: flagCommand,
            helpTopic: null
        };
    }

    if (flagCommand === COMMANDS.HELP && isKnownCommand(firstPositional)) {
        if (remainingPositionals.length > 0) {
            throw unexpectedArgumentsError(remainingPositionals);
        }

        return {
            command: COMMANDS.HELP,
            optionScope: firstPositional,
            helpTopic: firstPositional
        };
    }

    if (flagCommand === COMMANDS.VERSION && isOperationalCommand(firstPositional)) {
        if (remainingPositionals.length > 0) {
            throw unexpectedArgumentsError(remainingPositionals);
        }

        return {
            command: COMMANDS.VERSION,
            optionScope: firstPositional,
            helpTopic: null
        };
    }

    if (isKnownCommand(firstPositional)) {
        throw usageError(
            'CONFLICTING_COMMANDS',
            `Cannot combine command "${firstPositional}" with ${flagCommand}`
        );
    }

    throw usageError('UNKNOWN_COMMAND', `Unknown command: ${firstPositional}`);
}

function resolveHelpCommand(positionals) {
    if (positionals.length === 0) {
        return {
            command: COMMANDS.HELP,
            optionScope: COMMANDS.HELP,
            helpTopic: null
        };
    }

    const [topic, ...remainingPositionals] = positionals;
    if (!isKnownCommand(topic)) {
        throw usageError('UNKNOWN_COMMAND', `Unknown command: ${topic}`);
    }

    if (remainingPositionals.length > 0) {
        throw unexpectedArgumentsError(remainingPositionals);
    }

    return {
        command: COMMANDS.HELP,
        optionScope: topic,
        helpTopic: topic
    };
}

function deriveFlagCommand(values) {
    if (values.help && values.version) {
        throw usageError(
            'CONFLICTING_COMMANDS',
            'Cannot request both help and version in the same invocation'
        );
    }

    if (values.version) {
        return COMMANDS.VERSION;
    }

    if (values.help) {
        return COMMANDS.HELP;
    }

    return null;
}

function assertSupportedOptions(command, values) {
    const allowedOptions = COMMAND_OPTION_SUPPORT[command];

    for (const optionName of ['target', 'watchlist', 'concurrency']) {
        if (values[optionName] !== undefined && !allowedOptions.has(optionName)) {
            throw usageError(
                'UNSUPPORTED_OPTION',
                `Option --${optionName} is not supported for ${command}`
            );
        }
    }
}

function unexpectedArgumentsError(args) {
    const label = args.length === 1 ? 'argument' : 'arguments';
    return usageError('UNEXPECTED_ARGUMENTS', `Unexpected positional ${label}: ${args.join(', ')}`);
}

function isKnownCommand(command) {
    return Object.values(COMMANDS).includes(command);
}

function isOperationalCommand(command) {
    return [COMMANDS.RUN, COMMANDS.INIT, COMMANDS.STATUS].includes(command);
}

function normalizeConcurrencyOption(value, rawValue = value) {
    if (!Number.isInteger(value) || value <= 0) {
        throw usageError('INVALID_CONCURRENCY', `Invalid concurrency: ${rawValue}`);
    }

    return value;
}

function summarizeStatusTargets(targets) {
    const summary = {
        total_targets: targets.length,
        ready: 0,
        pending: 0,
        invalid: 0,
        attention_required: false,
        ready_target_names: [],
        pending_target_names: [],
        invalid_target_names: [],
        invalid_codes: {},
        last_initialized: 0,
        last_changed: 0,
        last_unchanged: 0,
        last_failed: 0,
        never_run: 0,
        last_initialized_target_names: [],
        last_changed_target_names: [],
        last_failed_target_names: [],
        never_run_target_names: []
    };

    for (const target of targets) {
        if (target.status === 'ready') {
            summary.ready++;
            summary.ready_target_names.push(target.name);
        } else if (target.status === 'pending') {
            summary.pending++;
            summary.pending_target_names.push(target.name);
        } else if (target.status === 'invalid') {
            summary.invalid++;
            summary.invalid_target_names.push(target.name);
            const invalidCodes = target.artifacts?.issues?.map((issue) => issue.code) ?? [];

            if (invalidCodes.length === 0 && target.error?.code) {
                invalidCodes.push(target.error.code);
            }

            for (const code of invalidCodes) {
                summary.invalid_codes[code] = (summary.invalid_codes[code] || 0) + 1;
            }
        }

        if (target.last_run?.status === STATUS.INITIALIZED) {
            summary.last_initialized++;
            summary.last_initialized_target_names.push(target.name);
        } else if (target.last_run?.status === STATUS.CHANGED) {
            summary.last_changed++;
            summary.last_changed_target_names.push(target.name);
        } else if (target.last_run?.status === STATUS.UNCHANGED) {
            summary.last_unchanged++;
        } else if (target.last_run?.status === STATUS.FAILED) {
            summary.last_failed++;
            summary.last_failed_target_names.push(target.name);
        } else if (target.last_run?.status === 'never') {
            summary.never_run++;
            summary.never_run_target_names.push(target.name);
        }
    }

    summary.attention_required = summary.pending > 0
        || summary.invalid > 0
        || summary.last_changed > 0
        || summary.last_failed > 0;
    return summary;
}

function createStatusTargetBase(name, url, paths) {
    return {
        name,
        url,
        files: buildFiles(paths)
    };
}

function createInvalidStatusTarget(baseTarget, error, extra = {}) {
    return {
        ...baseTarget,
        status: 'invalid',
        ...extra,
        error: serializeError(error)
    };
}

function buildNeverRunDetails() {
    return {
        status: 'never',
        at: null,
        success_at: null,
        change_at: null,
        error: null
    };
}

function buildLastRunDetails(state) {
    if (!state.last_run_at) {
        return buildNeverRunDetails();
    }

    return {
        status: deriveLastRunStatus(state),
        at: state.last_run_at,
        success_at: state.last_success_at,
        change_at: state.last_change_at,
        error: state.last_error
    };
}

function deriveLastRunStatus(state) {
    if (state.last_error && state.last_error.at === state.last_run_at) {
        return STATUS.FAILED;
    }

    if (state.stats.successes === 1 && state.stats.changes === 0) {
        return STATUS.INITIALIZED;
    }

    if (state.last_change_at === state.last_run_at) {
        return STATUS.CHANGED;
    }

    return STATUS.UNCHANGED;
}

async function readStatusArtifacts(runtime, state, paths) {
    const currentContent = await runtime.readTextFileIfExistsFn(paths.current);
    const previousContent = await runtime.readTextFileIfExistsFn(paths.previous);

    return {
        content: buildFailureContentDetails(currentContent, previousContent),
        currentContent,
        previousContent,
        artifacts: inspectStoredArtifacts(state, currentContent, previousContent)
    };
}

function inspectStoredArtifacts(state, currentContent, previousContent) {
    const current = buildArtifactFileDetails(currentContent, state?.current_hash ?? null);
    const previous = buildArtifactFileDetails(previousContent, state?.previous_hash ?? null);
    const issues = [];

    if (state === null) {
        if (current.exists) {
            issues.push(createArtifactIssue('ORPHANED_CURRENT_FILE', 'current.txt exists but state.json is missing'));
        }

        if (previous.exists) {
            issues.push(createArtifactIssue('ORPHANED_PREVIOUS_FILE', 'previous.txt exists but state.json is missing'));
        }
    } else {
        if (state.current_hash === null && current.exists) {
            issues.push(createArtifactIssue('CURRENT_FILE_UNEXPECTED', 'current.txt exists but state.current_hash is null'));
        }

        if (state.current_hash !== null && !current.exists) {
            issues.push(createArtifactIssue('CURRENT_FILE_MISSING', 'current.txt is missing for a state with current_hash'));
        }

        if (state.current_hash !== null && current.exists && current.matches_state_hash === false) {
            issues.push(createArtifactIssue('CURRENT_HASH_MISMATCH', 'current.txt content does not match state.current_hash'));
        }

        if (state.previous_hash === null && previous.exists) {
            issues.push(createArtifactIssue('PREVIOUS_FILE_UNEXPECTED', 'previous.txt exists but state.previous_hash is null'));
        }

        if (state.previous_hash !== null && !previous.exists) {
            issues.push(createArtifactIssue('PREVIOUS_FILE_MISSING', 'previous.txt is missing for a state with previous_hash'));
        }

        if (state.previous_hash !== null && previous.exists && previous.matches_state_hash === false) {
            issues.push(createArtifactIssue('PREVIOUS_HASH_MISMATCH', 'previous.txt content does not match state.previous_hash'));
        }
    }

    return {
        consistent: issues.length === 0,
        issues,
        current,
        previous
    };
}

function buildArtifactFileDetails(content, stateHash) {
    const exists = content !== null;
    const hash = exists ? computeHash(content) : null;

    return {
        exists,
        chars: exists ? content.length : null,
        hash,
        state_hash: stateHash,
        matches_state_hash: stateHash === null || hash === null ? null : hash === stateHash
    };
}

function createArtifactIssue(code, message) {
    return { code, message };
}

function createArtifactIntegrityError(targetName, artifacts) {
    return targetFailure(
        'STATUS_ARTIFACTS_INVALID',
        ERROR_TYPES.STATE,
        `Stored artifacts are inconsistent for target: ${targetName}`,
        {
            target: targetName,
            issues: artifacts.issues
        }
    );
}
