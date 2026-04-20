import java.util.Optional;

/**
 * A sample Java class for parser testing
 */
public class ConfigService {
    private String name;
    private int value;

    public ConfigService(String name, int value) {
        this.name = name;
        this.value = value;
    }

    public boolean validate() {
        return name != null && !name.isEmpty() && value > 0;
    }

    public String getName() {
        return name;
    }
}

interface Validator {
    boolean validate();
}

enum Status {
    ACTIVE,
    INACTIVE,
    PENDING
}

class AdvancedService extends ConfigService {
    public AdvancedService(String name, int value) {
        super(name, value);
    }
}
