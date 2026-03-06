package demo;

import static java.util.Collections.emptyList;
import static java.util.Objects.requireNonNull;

class Parser {
    int parse(String s) {
        return s.length();
    }

    int parse(String s, int base) {
        return Integer.parseInt(s, base);
    }

    static class Outer {
        static class Inner {
            static int compute() {
                return 42;
            }
        }
    }

    int run(String raw) {
        String s = requireNonNull(raw);
        emptyList();
        return parse(s) + parse(s, 10) + Outer.Inner.compute();
    }
}
