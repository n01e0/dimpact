package demo;

import java.util.function.Function;

class ExtractionFpPointsV4 {
    interface Router {
        String dispatch(String payload);
    }

    static class DefaultRouter implements Router {
        public String dispatch(String payload) {
            return payload.trim();
        }
    }

    static String invoke(Function<String, String> fn, String payload) {
        return fn.apply(payload);
    }

    String run(Router router, String payload) {
        String fake = "router.missing(payload)";
        String block = """
            helper();
            Router.dispatch("x");
            """;
        // router.missing(payload)
        /* invoke(router::missing, payload); */

        Function<String, String> bound = router::dispatch;
        String viaFn = invoke(bound, payload);
        String direct = router.dispatch(payload);
        return viaFn + direct + block + fake;
    }
}
