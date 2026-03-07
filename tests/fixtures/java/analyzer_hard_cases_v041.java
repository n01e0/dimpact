package demo;

import static java.util.Collections.emptyList;
import static java.util.Objects.requireNonNull;

class Engine {
    static class Nested {
        static int eval(String s) {
            return s.length();
        }
    }

    int parse(String s) {
        return s.length();
    }

    int parse(String s, int radix) {
        return Integer.parseInt(s, radix);
    }

    int run(String raw) {
        String v = requireNonNull(raw);
        emptyList();
        return parse(v) + parse(v, 10) + Engine.Nested.eval(v);
    }
}
