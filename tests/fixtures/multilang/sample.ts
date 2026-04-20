/**
 * A sample TypeScript module for parser testing
 */

import { readFile } from 'fs/promises';

interface Validator {
    validate(): boolean;
}

interface Config {
    name: string;
    value: number;
}

class ConfigService implements Validator {
    private config: Config;

    constructor(config: Config) {
        this.config = config;
    }

    validate(): boolean {
        return this.config.name.length > 0 && this.config.value > 0;
    }

    getName(): string {
        return this.config.name;
    }
}

enum Status {
    Active = 'active',
    Inactive = 'inactive',
    Pending = 'pending',
}

type ConfigResult = {
    status: Status;
    message: string;
};

/**
 * Process a configuration and return its status
 */
function processConfig(config: Config): ConfigResult {
    const service = new ConfigService(config);
    if (service.validate()) {
        return { status: Status.Active, message: service.getName() };
    }
    return { status: Status.Inactive, message: 'invalid' };
}

export const MAX_RETRIES = 3;

export const fetchConfig = async (path: string): Promise<Config> => {
    const data = await readFile(path, 'utf-8');
    return JSON.parse(data);
};
