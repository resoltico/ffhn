import { spawn } from 'node:child_process';
import { DEFAULTS, ERROR_TYPES } from '../core/constants.js';
import { dependencyError, targetFailure } from '../utils/error.js';

export function executeCommand(command, args = [], options = {}) {
    return new Promise((resolve, reject) => {
        const {
            timeoutMs = 0,
            input = null,
            ...spawnOptions
        } = options;
        const child = spawn(command, args, {
            stdio: ['pipe', 'pipe', 'pipe'],
            ...spawnOptions
        });
        let timeoutId = null;

        let stdout = '';
        let stderr = '';

        child.stdout?.on('data', (data) => {
            stdout += data.toString();
        });

        child.stderr?.on('data', (data) => {
            stderr += data.toString();
        });

        child.on('close', (code, signal) => {
            if (timeoutId) {
                clearTimeout(timeoutId);
            }

            if (code === 0) {
                resolve({
                    stdout,
                    stderr,
                    exitCode: code
                });
            } else {
                reject(targetFailure(
                    'COMMAND_FAILED',
                    ERROR_TYPES.DEPENDENCY,
                    `Command "${command}" failed`,
                    {
                        command,
                        args,
                        exit_code: code,
                        signal,
                        stdout,
                        stderr
                    }
                ));
            }
        });

        child.on('error', (error) => {
            if (timeoutId) {
                clearTimeout(timeoutId);
            }

            if (error.code === 'ENOENT') {
                reject(dependencyError(
                    'COMMAND_NOT_FOUND',
                    `Required command not found: ${command}`,
                    {
                        command
                    },
                    error
                ));
                return;
            }

            reject(targetFailure(
                'COMMAND_SPAWN_FAILED',
                ERROR_TYPES.DEPENDENCY,
                `Failed to execute command "${command}"`,
                {
                    command,
                    args
                },
                error
            ));
        });

        if (input !== null) {
            child.stdin?.end(input);
        } else {
            child.stdin?.end();
        }

        if (timeoutMs > 0) {
            timeoutId = setTimeout(() => {
                child.kill('SIGTERM');
                reject(targetFailure(
                    'COMMAND_TIMEOUT',
                    ERROR_TYPES.DEPENDENCY,
                    `Command "${command}" timed out after ${timeoutMs}ms`,
                    {
                        command,
                        args,
                        timeout_ms: timeoutMs
                    }
                ));
            }, timeoutMs);
        }
    });
}

export async function executeHtmlcut(input, htmlcutConfig, options = {}) {
    const args = ['-'];

    args.push('--from', htmlcutConfig.from);
    args.push('--to', htmlcutConfig.to);
    args.push('--pattern', htmlcutConfig.pattern);

    if (htmlcutConfig.flags) {
        args.push('--flags', htmlcutConfig.flags);
    }

    if (htmlcutConfig.capture !== 'inner') {
        args.push('--capture', htmlcutConfig.capture);
    }

    if (htmlcutConfig.all) {
        args.push('--all');
    }

    if (options.baseUrl) {
        args.push('--base-url', options.baseUrl);
    }

    args.push('--format', 'text');

    try {
        return await executeCommand(DEFAULTS.HTMLCUT_COMMAND, args, {
            timeoutMs: options.timeoutMs || DEFAULTS.HTMLCUT_TIMEOUT_MS,
            input
        });
    } catch (error) {
        if (error.code === 'COMMAND_NOT_FOUND') {
            throw dependencyError(
                'HTMLCUT_NOT_FOUND',
                'HTMLCut binary not found in PATH',
                null,
                error
            );
        }

        throw targetFailure(
            'HTMLCUT_FAILED',
            ERROR_TYPES.EXTRACT,
            'HTMLCut execution failed',
            {
                base_url: options.baseUrl || null
            },
            error
        );
    }
}
