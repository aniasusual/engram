"""A sample Python module for parser testing"""

import os
from typing import Optional


class Config:
    """Configuration holder"""

    def __init__(self, name: str, value: int):
        self.name = name
        self.value = value

    def validate(self) -> bool:
        """Check if config is valid"""
        return len(self.name) > 0 and self.value > 0


class AdvancedConfig(Config):
    """Extended configuration with extra features"""

    def __init__(self, name: str, value: int, debug: bool = False):
        super().__init__(name, value)
        self.debug = debug

    def validate(self) -> bool:
        """Override validation with additional checks"""
        return super().validate() and isinstance(self.debug, bool)


def process_config(config: Config) -> Optional[str]:
    """Process a config and return result"""
    if config.validate():
        return f"Processed: {config.name}"
    return None


MAX_RETRIES = 3
