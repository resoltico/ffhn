import { INIT } from './constants.js';
import { loadTargetConfig } from './config.js';
import { buildTargetPaths } from '../utils/path.js';
import { validateTargetName } from '../utils/validation.js';

export async function loadTarget(discoveredTarget) {
    validateTargetName(discoveredTarget.name);

    const config = await loadTargetConfig(discoveredTarget.configPath);
    const paths = buildTargetPaths(discoveredTarget.dir);

    return Object.freeze({
        name: discoveredTarget.name,
        dir: discoveredTarget.dir,
        configPath: discoveredTarget.configPath,
        url: config.url,
        request: config.request,
        extract: config.extract,
        paths
    });
}

export function createTargetTemplate() {
    return INIT.TARGET_CONFIG_TEMPLATE;
}
