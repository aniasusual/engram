# A sample Ruby module for parser testing

module Sample
  class Config
    attr_reader :name, :value

    def initialize(name, value)
      @name = name
      @value = value
    end

    def validate
      !name.nil? && !name.empty? && value > 0
    end
  end

  class AdvancedConfig < Config
    def initialize(name, value, debug: false)
      super(name, value)
      @debug = debug
    end

    def validate
      super && @debug.is_a?(TrueClass) || @debug.is_a?(FalseClass)
    end
  end

  def self.process_config(config)
    if config.validate
      "Processed: #{config.name}"
    end
  end
end
