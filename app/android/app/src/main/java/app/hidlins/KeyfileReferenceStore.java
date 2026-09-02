package app.hidlins;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.FileNotFoundException;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/** Persists non-secret SAF references and exposes open descriptors without copying keyfile bytes. */
final class KeyfileReferenceStore {
    private static final String PREFIX = "keyfile.";
    private final ContentResolver resolver;
    private final SharedPreferences preferences;
    private final Map<String, ParcelFileDescriptor> active = new HashMap<>();

    KeyfileReferenceStore(Context context) {
        resolver = context.getContentResolver();
        preferences = context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE);
    }

    PlatformEnvelope.Result save(Uri uri, String displayName) {
        if (uri == null || !ContentResolver.SCHEME_CONTENT.equals(uri.getScheme())) {
            return PlatformEnvelope.failure("invalid-keyfile-uri");
        }
        boolean grantRetained = false;
        try {
            resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            grantRetained = true;
            String id = UUID.randomUUID().toString();
            if (!preferences.edit().putString(PREFIX + id, uri.toString()).commit()) {
                releaseGrant(uri);
                return PlatformEnvelope.failure("keyfile-reference-not-persisted");
            }
            Map<String, Object> value = new HashMap<>();
            value.put("reference", id);
            value.put("displayName", displayName == null || displayName.isEmpty() ? "keyfile" : displayName);
            return PlatformEnvelope.success(value);
        } catch (SecurityException error) {
            if (grantRetained) releaseGrant(uri);
            return PlatformEnvelope.denied();
        } catch (RuntimeException error) {
            if (grantRetained) releaseGrant(uri);
            return PlatformEnvelope.failure("keyfile-reference-failed");
        }
    }

    private void releaseGrant(Uri uri) {
        try {
            resolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
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
