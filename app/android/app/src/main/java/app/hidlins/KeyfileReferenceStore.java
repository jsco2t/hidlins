package app.hidlins;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.UriPermission;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.FileNotFoundException;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

/** Persists non-secret SAF references and exposes open descriptors without copying keyfile bytes. */
final class KeyfileReferenceStore {
    private static final String PREFIX = "keyfile.";
    private static final Object SAVE_LOCK = new Object();
    // Only grants acquired by undelivered saves are eligible for rollback.
    private static final Set<String> pendingGrants = new HashSet<>();
    private final ContentResolver resolver;
    private final SharedPreferences preferences;
    private final Map<String, ParcelFileDescriptor> active = new HashMap<>();

    KeyfileReferenceStore(Context context) {
        resolver = context.getContentResolver();
        preferences = context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE);
    }

    PlatformEnvelope.Result save(Uri uri, String displayName) {
        PendingReference pending = savePending(uri, displayName);
        pending.complete(true);
        return pending.result;
    }

    final class PendingReference {
        final PlatformEnvelope.Result result;
        private final Uri uri;
        private final String id;

        PendingReference(PlatformEnvelope.Result result, Uri uri, String id) {
            this.result = result;
            this.uri = uri;
            this.id = id;
        }

        void complete(boolean delivered) {
            if (id == null) return;
            synchronized (SAVE_LOCK) {
                if (delivered) pendingGrants.remove(uri.toString());
                else rollback(uri, id);
            }
        }
    }

    PendingReference savePending(Uri uri, String displayName) {
        if (uri == null || !ContentResolver.SCHEME_CONTENT.equals(uri.getScheme())) {
            return new PendingReference(PlatformEnvelope.failure("invalid-keyfile-uri"), null, null);
        }
        synchronized (SAVE_LOCK) {
            String id = null;
            try {
                boolean existing = false;
                for (UriPermission permission : resolver.getPersistedUriPermissions()) {
                    if (permission.getUri().equals(uri) && permission.isReadPermission()) existing = true;
                }
                resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
                if (!existing) pendingGrants.add(uri.toString());
                id = UUID.randomUUID().toString();
                if (!preferences.edit().putString(PREFIX + id, uri.toString()).commit()) {
                    rollback(uri, id);
                    return new PendingReference(PlatformEnvelope.failure("keyfile-reference-not-persisted"), null, null);
                }
                Map<String, Object> value = new HashMap<>();
                value.put("reference", id);
                value.put("displayName", displayName == null || displayName.isEmpty() ? "keyfile" : displayName);
                return new PendingReference(PlatformEnvelope.success(value), uri, id);
            } catch (SecurityException error) {
                rollback(uri, id);
                return new PendingReference(PlatformEnvelope.denied(), null, null);
            } catch (RuntimeException error) {
                rollback(uri, id);
                return new PendingReference(PlatformEnvelope.failure("keyfile-reference-failed"), null, null);
            }
        }
    }

    private void rollback(Uri uri, String id) {
        if (id != null) {
            try {
                if (uri.toString().equals(preferences.getString(PREFIX + id, null))) {
                    preferences.edit().remove(PREFIX + id).commit();
                }
            } catch (RuntimeException ignored) { }
        }
        if (!pendingGrants.contains(uri.toString())) return;
        try {
            // Another activity may have saved the same URI while this result
            // was awaiting main-thread delivery. Never revoke its reference.
            for (Map.Entry<String, ?> entry : preferences.getAll().entrySet()) {
                if (entry.getKey().startsWith(PREFIX) && uri.toString().equals(entry.getValue())) return;
            }
            resolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            pendingGrants.remove(uri.toString());
        } catch (RuntimeException ignored) { }
    }

    PlatformEnvelope.Result resolve(String id) {
        String stored = preferences.getString(PREFIX + id, null);
        if (stored == null) return PlatformEnvelope.stale();
        try {
            ParcelFileDescriptor descriptor = resolver.openFileDescriptor(Uri.parse(stored), "r");
            if (descriptor == null) return PlatformEnvelope.stale();
            releaseDescriptor(id);
            active.put(id, descriptor);
            return PlatformEnvelope.success("/proc/self/fd/" + descriptor.getFd());
        } catch (SecurityException error) {
            return PlatformEnvelope.denied();
        } catch (FileNotFoundException error) {
            return PlatformEnvelope.stale();
        } catch (RuntimeException error) {
            return PlatformEnvelope.failure("keyfile-open-failed");
        }
    }

    PlatformEnvelope.Result release(String id) {
        releaseDescriptor(id);
        return PlatformEnvelope.success(null);
    }

    void closeAll() {
        for (ParcelFileDescriptor descriptor : active.values()) {
            try { descriptor.close(); } catch (java.io.IOException ignored) { }
        }
        active.clear();
    }

    private void releaseDescriptor(String id) {
        ParcelFileDescriptor descriptor = active.remove(id);
        if (descriptor != null) {
            try { descriptor.close(); } catch (java.io.IOException ignored) { }
        }
    }
}
