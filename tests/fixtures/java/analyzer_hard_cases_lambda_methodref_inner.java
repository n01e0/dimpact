package demo;

import java.util.List;
import java.util.function.Function;

class Outer {
    static class Inner {
        static int parse(String s) {
            return Integer.parseInt(s);
        }

        int compute(String s) {
            return parse(s);
        }
    }
}

class Flow {
    int parse(String s) {
        return Integer.parseInt(s);
    }

    int run(List<String> xs) {
        Function<String, Integer> f = this::parse;
        Function<String, Integer> g = (v) -> parse(v);
        Outer.Inner inner = new Outer.Inner();
        Function<String, Integer> h = inner::compute;

        return xs.stream()
            .map(v -> g.apply(v))
            .map(v -> h.apply(v))
            .map(Outer.Inner::parse)
            .map(v -> Outer.Inner.parse(v))
            .findFirst()
            .orElseGet(() -> f.apply("0"));
    }
}
