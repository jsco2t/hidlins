package app.hidlins;

import static app.hidlins.DeviceAssertions.assertEquals;
import static app.hidlins.DeviceAssertions.assertFalse;
import static app.hidlins.DeviceAssertions.assertNotNull;
import static app.hidlins.DeviceAssertions.assertTrue;

import android.app.Instrumentation;
import android.content.ContentResolver;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.UriPermission;
import android.net.Uri;
import android.os.Bundle;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

/** Real provider grants and preference-failure controls; no keyfile bytes are persisted. */
final class SafKeyfileRegressions {
    private static final Uri PROVIDER = Uri.parse("content://" + HidlinsControlledDocumentProvider.AUTHORITY);

    private static boolean persistedRead(ContentResolver resolver, Uri uri) {
        for (UriPermission permission : resolver.getPersistedUriPermissions()) {
            if (permission.getUri().equals(uri) && permission.isReadPermission()) return true;
        }
        return false;
    }

    private static final class FailingPreferences implements SharedPreferences {
        final SharedPreferences delegate;
        final boolean throwsOnCommit;
        FailingPreferences(SharedPreferences delegate, boolean throwsOnCommit) {
            this.delegate = delegate;
            this.throwsOnCommit = throwsOnCommit;
        }
        @Override public Map<String, ?> getAll() { return delegate.getAll(); }
        @Override public String getString(String key, String fallback) { return delegate.getString(key, fallback); }
        @Override public Set<String> getStringSet(String key, Set<String> fallback) { return delegate.getStringSet(key, fallback); }
        @Override public int getInt(String key, int fallback) { return delegate.getInt(key, fallback); }
        @Override public long getLong(String key, long fallback) { return delegate.getLong(key, fallback); }
        @Override public float getFloat(String key, float fallback) { return delegate.getFloat(key, fallback); }
        @Override public boolean getBoolean(String key, boolean fallback) { return delegate.getBoolean(key, fallback); }
        @Override public boolean contains(String key) { return delegate.contains(key); }
        @Override public void registerOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {
            delegate.registerOnSharedPreferenceChangeListener(listener);
        }
        @Override public void unregisterOnSharedPreferenceChangeListener(OnSharedPreferenceChangeListener listener) {
            delegate.unregisterOnSharedPreferenceChangeListener(listener);
        }
        @Override public Editor edit() {
            return new Editor() {
                @Override public Editor putString(String key, String value) { return this; }
                @Override public Editor putStringSet(String key, Set<String> value) { return this; }
                @Override public Editor putInt(String key, int value) { return this; }
                @Override public Editor putLong(String key, long value) { return this; }
                @Override public Editor putFloat(String key, float value) { return this; }
                @Override public Editor putBoolean(String key, boolean value) { return this; }
                @Override public Editor remove(String key) { return this; }
                @Override public Editor clear() { return this; }
                @Override public boolean commit() {
                    if (throwsOnCommit) throw new IllegalStateException("fixture commit failure");
                    return false;
                }
                @Override public void apply() { throw new AssertionError("reference persistence must report failure"); }
            };
        }
    }

    static void failedSavePreservesEarlierGrant(Instrumentation instrumentation) throws Exception {
        checkFailedSave(instrumentation, true, false);
        checkFailedSave(instrumentation, true, true);
    }

    static void failedNewSaveLeavesNoGrant(Instrumentation instrumentation) throws Exception {
        checkFailedSave(instrumentation, false, false);
        checkFailedSave(instrumentation, false, true);
    }

    private static void checkFailedSave(Instrumentation instrumentation, boolean earlier, boolean throwsOnCommit)
            throws Exception {
        Context context = instrumentation.getTargetContext();
        ContentResolver resolver = context.getContentResolver();
        SharedPreferences preferences = context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE);
        String id = UUID.randomUUID().toString();
        Uri uri = PROVIDER.buildUpon().appendPath(id).build();
        Bundle arguments = new Bundle();
        arguments.putString("phase", "none");
        Bundle configured = resolver.call(PROVIDER, "configure", id, arguments);
        assertNotNull(configured);
        assertTrue(configured.getBoolean("configured"));
        resolver.call(PROVIDER, "grant", id, null);
        assertFalse(persistedRead(resolver, uri));
        String originalReference = null;
        try {
            if (earlier) {
                PlatformEnvelope.Result saved = new KeyfileReferenceStore(context).save(uri, "original.key");
                assertEquals(PlatformEnvelope.Status.SUCCESS, saved.status());
                Object body = saved.envelope().get("value");
                assertTrue(body instanceof Map<?, ?>);
                originalReference = (String) ((Map<?, ?>) body).get("reference");
                assertNotNull(originalReference);
                assertEquals(uri.toString(), preferences.getString("keyfile." + originalReference, null));
                assertTrue(persistedRead(resolver, uri));
            }
            Map<String, ?> before = preferences.getAll();
            Context failingContext = new ContextWrapper(context) {
                @Override public SharedPreferences getSharedPreferences(String name, int mode) {
                    assertEquals("hidlins-keyfiles", name);
                    return new FailingPreferences(preferences, throwsOnCommit);
                }
            };
            PlatformEnvelope.Result failed = new KeyfileReferenceStore(failingContext).save(uri, "failed.key");
            assertEquals(PlatformEnvelope.Status.FAILURE, failed.status());
            assertEquals(before, preferences.getAll());
            assertEquals(earlier, persistedRead(resolver, uri));
            if (originalReference != null) {
                assertEquals(uri.toString(), preferences.getString("keyfile." + originalReference, null));
            }
        } finally {
            if (originalReference != null) assertTrue(preferences.edit().remove("keyfile." + originalReference).commit());
            if (persistedRead(resolver, uri)) {
                resolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            }
            resolver.call(PROVIDER, "remove", id, null);
        }
    }
}
