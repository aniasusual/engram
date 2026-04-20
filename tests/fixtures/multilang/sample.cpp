#include <string>
#include <iostream>

namespace sample {

class Config {
public:
    std::string name;
    int value;

    Config(const std::string& n, int v) : name(n), value(v) {}

    bool validate() const {
        return !name.empty() && value > 0;
    }
};

struct Point {
    double x;
    double y;
};

enum class Status {
    Active,
    Inactive,
    Pending
};

} // namespace sample

void process_config(const sample::Config& config) {
    if (config.validate()) {
        std::cout << "Valid: " << config.name << std::endl;
    }
}
