/**
 * A sample JavaScript module for parser testing
 */

import { readFile } from 'fs';

class ConfigService {
    constructor(config) {
        this.config = config;
    }

    validate() {
        return this.config.name.length > 0 && this.config.value > 0;
    }

    getName() {
        return this.config.name;
    }
}

function processConfig(config) {
    const service = new ConfigService(config);
    if (service.validate()) {
        return { status: 'active', message: service.getName() };
    }
    return { status: 'inactive', message: 'invalid' };
}

export const fetchConfig = async (path) => {
    const data = await readFile(path, 'utf-8');
    return JSON.parse(data);
};
