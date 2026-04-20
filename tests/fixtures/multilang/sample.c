#include <stdio.h>
#include <stdlib.h>

/* A sample C file for parser testing */

struct Config {
    char* name;
    int value;
};

enum Status {
    ACTIVE,
    INACTIVE,
    PENDING
};

typedef struct Config ConfigAlias;

int validate_config(struct Config* config) {
    return config->name != NULL && config->value > 0;
}

void process_config(struct Config* config) {
    if (validate_config(config)) {
        printf("Valid: %s\n", config->name);
    }
}
