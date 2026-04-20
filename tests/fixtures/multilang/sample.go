// Package sample provides a sample Go module for parser testing
package sample

import "fmt"

// Config holds configuration values
type Config struct {
	Name  string
	Value int
}

// Validator defines the validation interface
type Validator interface {
	Validate() bool
}

// Validate checks if the config is valid
func (c *Config) Validate() bool {
	return len(c.Name) > 0 && c.Value > 0
}

// ProcessConfig processes a configuration and returns its status
func ProcessConfig(config *Config) string {
	if config.Validate() {
		return fmt.Sprintf("Processed: %s", config.Name)
	}
	return "invalid"
}

// MaxRetries is the maximum number of retry attempts
const MaxRetries = 3
