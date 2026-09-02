package app.hidlins;

import java.util.HashMap;
import java.util.Map;
import java.util.regex.Pattern;

/** Stable, secret-free result envelopes shared with Dart platform adapters. */
final class PlatformEnvelope {
    enum Status { SUCCESS, CANCELED, DENIED, STALE, UNSUPPORTED, FAILURE }

    static final class Result {
        private final Status status;
        private final Map<String, Object> envelope;

        Result(Status status, Map<String, Object> envelope) {
            this.status = status;
            this.envelope = envelope;
        }

        Status status() { return status; }
        Map<String, Object> envelope() { return envelope; }
    }

    private static final Pattern SAFE_CODE = Pattern.compile("^[a-zA-Z0-9._-]{1,64}$");

    private PlatformEnvelope() {}

    static Result success(Object value) { return result(Status.SUCCESS, "success", value, null); }
    static Result canceled() { return result(Status.CANCELED, "canceled", null, null); }
    static Result denied() { return result(Status.DENIED, "denied", null, null); }
    static Result stale() { return result(Status.STALE, "stale", null, null); }
    static Result unsupported() { return result(Status.UNSUPPORTED, "unsupported", null, null); }
    static Result failure(String code) {
        String safe = SAFE_CODE.matcher(code).matches() ? code : "platform-error";
        return result(Status.FAILURE, "failure", null, safe);
    }

    private static Result result(Status status, String wireStatus, Object value, String code) {
        Map<String, Object> envelope = new HashMap<>();
        envelope.put("status", wireStatus);
        if (value != null) envelope.put("value", value);
        if (code != null) envelope.put("code", code);
        return new Result(status, envelope);
    }
}
