package demo;

import java.util.List;
import java.util.function.BiFunction;
import java.util.function.Function;

class OverloadLab {
    int parse(String s) {
        return Integer.parseInt(s);
    }

    int parse(CharSequence s) {
        return Integer.parseInt(s.toString());
    }

    int parse(String s, int base) {
        return Integer.parseInt(s, base);
    }

    static int parseStatic(String s) {
        return Integer.parseInt(s);
    }

    int run(List<String> xs) {
        Function<String, Integer> f1 = this::parse;
        Function<String, Integer> f2 = OverloadLab::parseStatic;
        BiFunction<String, Integer, Integer> f3 = this::parse;
        Function<String, Integer> f4 = v -> parse(v);
        Function<String, Integer> f5 = v -> parse((CharSequence) v);

        return xs.stream()
            .map(v -> f1.apply(v))
            .map(v -> f2.apply(v))
            .map(v -> f3.apply(v, 10))
            .map(v -> f4.apply(v))
            .map(v -> f5.apply(v))
            .findFirst()
            .orElseGet(() -> this.parse("0"));
    }
}
