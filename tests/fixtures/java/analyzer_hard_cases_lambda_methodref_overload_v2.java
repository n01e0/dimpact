package demo;

import java.util.List;
import java.util.function.BiFunction;
import java.util.function.Function;

class JavaOverloadLabV2 {
    int decode(String s) {
        return Integer.parseInt(s);
    }

    int decode(CharSequence s) {
        return Integer.parseInt(s.toString());
    }

    int decode(String s, int base) {
        return Integer.parseInt(s, base);
    }

    static int decodeStatic(String s) {
        return Integer.parseInt(s);
    }

    int run(List<String> xs) {
        JavaOverloadLabV2 codec = new JavaOverloadLabV2();

        Function<String, Integer> f1 = this::decode;
        Function<String, Integer> f2 = codec::decode;
        Function<String, Integer> f3 = JavaOverloadLabV2::decodeStatic;
        BiFunction<String, Integer, Integer> f4 = codec::decode;
        Function<String, Integer> f5 = v -> decode(v);
        Function<String, Integer> f6 = v -> decode((CharSequence) v);
        BiFunction<String, Integer, Integer> f7 = (v, b) -> decode(v, b);

        return xs.stream()
            .map(v -> f1.apply(v))
            .map(v -> f2.apply(v))
            .map(v -> f3.apply(v))
            .map(v -> f4.apply(v, 10))
            .map(v -> f5.apply(v))
            .map(v -> f6.apply(v))
            .map(v -> f7.apply(v, 16))
            .findFirst()
            .orElseGet(() -> this.decode("0"));
    }
}
