package app.hidlins;

import java.util.Arrays;
import java.util.Objects;

/** The five assertion forms used by the platform device tests; no secret values in errors. */
final class DeviceAssertions {
    private DeviceAssertions() {}

    static void assertTrue(boolean condition) {
        if (!condition) {
            throw new AssertionError("expected true");
        }
    }

    static void assertFalse(boolean condition) {
        assertTrue(!condition);
    }

    static void assertNotNull(Object value) {
        if (value == null) {
            throw new AssertionError("expected a value");
        }
    }

    static void assertEquals(Object expected, Object actual) {
        if (!Objects.equals(expected, actual)) {
            throw new AssertionError("values differ");
        }
    }

    static void assertArrayEquals(byte[] expected, byte[] actual) {
        if (!Arrays.equals(expected, actual)) {
            throw new AssertionError("bytes differ");
        }
    }
}
