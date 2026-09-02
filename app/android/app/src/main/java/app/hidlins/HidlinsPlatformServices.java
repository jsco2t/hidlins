package app.hidlins;

import android.app.Activity;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;
import io.flutter.plugin.common.BinaryMessenger;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.MethodChannel;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.util.HashMap;
import java.util.Map;

/** Narrow Android mechanism adapter. Vault and lock policy remain in Rust. */
final class HidlinsPlatformServices {
    private static final int PICK_VAULT = 0x4849;
    private static final int PICK_KEYFILE = 0x484b;
    private final MainActivity activity;
    private final AppStorage storage;
    private final ClipboardController clipboard;
    private final KeyfileReferenceStore keyfiles;
    private final SnapshotCoverController cover;
    private MethodChannel lifecycle;
    private MethodChannel.Result pendingPicker;
    private int pendingRequest;

    HidlinsPlatformServices(MainActivity activity, SnapshotCoverController cover) {
        this.activity = activity;
        this.cover = cover;
        storage = new AppStorage(activity);
        clipboard = new ClipboardController(activity);
        keyfiles = new KeyfileReferenceStore(activity);
    }

    void register(BinaryMessenger messenger) {
        lifecycle = new MethodChannel(messenger, "app.hidlins/lifecycle");
        lifecycle.setMethodCallHandler(this::handleLifecycle);
        channel(messenger, "app.hidlins/paths", this::handlePaths);
        channel(messenger, "app.hidlins/clipboard", this::handleClipboard);
        channel(messenger, "app.hidlins/vault_import", this::handleVaultImport);
        channel(messenger, "app.hidlins/keyfile", this::handleKeyfile);
    }

    private static void channel(
            BinaryMessenger messenger, String name, MethodChannel.MethodCallHandler handler) {
        new MethodChannel(messenger, name).setMethodCallHandler(handler);
    }

    void reportLifecycle(String state) {
        if (!"resumed".equals(state)) cover.show(activity);
        if (lifecycle != null) {
            Map<String, Object> arguments = new HashMap<>();
            arguments.put("state", state);
            lifecycle.invokeMethod("reportState", arguments);
        }
    }

    private void handleLifecycle(MethodCall call, MethodChannel.Result result) {
        Map<?, ?> arguments = call.arguments instanceof Map<?, ?> map ? map : null;
        Object state = arguments == null ? null : arguments.get("state");
        Object acknowledged = arguments == null ? null : arguments.get("acknowledged");
        Object lockState = arguments == null ? null : arguments.get("lockState");
        if (!"reportState".equals(call.method)
                || !Boolean.TRUE.equals(acknowledged)
                || !(state instanceof String)
                || !("locked".equals(lockState) || "unlocked".equals(lockState))) {
            result.success(PlatformEnvelope.failure("invalid-lifecycle-ack").envelope());
            return;
        }
        if ("resumed".equals(state)) cover.hide();
        result.success(PlatformEnvelope.success(null).envelope());
    }

    private void handlePaths(MethodCall call, MethodChannel.Result result) {
        if (!"applicationSupportPath".equals(call.method)) {
            result.success(PlatformEnvelope.unsupported().envelope());
            return;
        }
        try {
            result.success(PlatformEnvelope.success(storage.prepare().getAbsolutePath()).envelope());
        } catch (IOException error) {
            result.success(PlatformEnvelope.failure("app-support-unavailable").envelope());
        }
    }

    private void handleClipboard(MethodCall call, MethodChannel.Result result) {
        Map<?, ?> arguments = call.arguments instanceof Map<?, ?> map ? map : null;
        Object transfer = arguments == null ? null : arguments.get("transferId");
        Object ttl = arguments == null ? null : arguments.get("ttlSeconds");
        if (!"copySecret".equals(call.method) || !(transfer instanceof String id)
                || !(ttl instanceof Integer seconds)) {
            result.success(PlatformEnvelope.failure("invalid-clipboard-request").envelope());
            return;
        }
        PlatformEnvelope.Result envelope = clipboard.consume(id, seconds)
                ? PlatformEnvelope.success(null)
                : PlatformEnvelope.failure("clipboard-transfer-failed");
        result.success(envelope.envelope());
    }

    private void handleVaultImport(MethodCall call, MethodChannel.Result result) {
        if (!"pickVault".equals(call.method)) {
            result.success(PlatformEnvelope.unsupported().envelope());
            return;
        }
        launchPicker(PICK_VAULT, "application/octet-stream", result);
    }

    private void handleKeyfile(MethodCall call, MethodChannel.Result result) {
        switch (call.method) {
            case "pickReference" -> launchPicker(PICK_KEYFILE, "application/octet-stream", result);
            case "resolveReference" -> result.success(keyfiles.resolve(reference(call)).envelope());
            case "releaseReference" -> result.success(keyfiles.release(reference(call)).envelope());
            default -> result.success(PlatformEnvelope.unsupported().envelope());
        }
    }

    private static String reference(MethodCall call) {
        if (!(call.arguments instanceof Map<?, ?> arguments)) return "";
        Object reference = arguments.get("reference");
        return reference instanceof String value ? value : "";
    }

    @SuppressWarnings("deprecation")
    private void launchPicker(int request, String type, MethodChannel.Result result) {
        if (!beginPicker(request, result)) {
            result.success(PlatformEnvelope.failure("picker-busy").envelope());
            return;
        }
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType(type)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION
                        | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        try {
            activity.startActivityForResult(intent, request);
        } catch (RuntimeException error) {
            completePending(PlatformEnvelope.failure("picker-launch-failed"));
        }
    }

    boolean beginPicker(int request, MethodChannel.Result result) {
        if (pendingPicker != null) return false;
        pendingPicker = result;
        pendingRequest = request;
        return true;
    }

    boolean onActivityResult(int requestCode, int resultCode, Intent data) {
        if (pendingPicker == null || requestCode != pendingRequest) return false;
        if (resultCode != Activity.RESULT_OK || data == null || data.getData() == null) {
            completePending(PlatformEnvelope.canceled());
            return true;
        }
        MethodChannel.Result result = takePending();
        Uri uri = data.getData();
        String displayName = displayName(uri);
        if (requestCode == PICK_KEYFILE) {
            result.success(keyfiles.save(uri, displayName).envelope());
            return true;
        }
        try {
            InputStream input = activity.getContentResolver().openInputStream(uri);
            if (input == null) {
                result.success(PlatformEnvelope.stale().envelope());
            } else {
                File imported = storage.importVault(input, displayName);
                Map<String, Object> value = new HashMap<>();
                value.put("sourceReference", imported.getAbsolutePath());
                value.put("displayName", imported.getName().replaceFirst("\\.kdbx$", ""));
                result.success(PlatformEnvelope.success(value).envelope());
            }
        } catch (SecurityException error) {
            result.success(PlatformEnvelope.denied().envelope());
        } catch (IOException | RuntimeException error) {
            result.success(PlatformEnvelope.failure("document-access-failed").envelope());
        }
        return true;
    }

    private MethodChannel.Result takePending() {
        MethodChannel.Result result = pendingPicker;
        pendingPicker = null;
        pendingRequest = 0;
        return result;
    }

    private void completePending(PlatformEnvelope.Result envelope) {
        MethodChannel.Result result = takePending();
        if (result != null) result.success(envelope.envelope());
    }

    private String displayName(Uri uri) {
        ContentResolver resolver = activity.getContentResolver();
        try (Cursor cursor = resolver.query(uri, new String[]{OpenableColumns.DISPLAY_NAME},
                null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null && !name.isEmpty()) return name;
            }
        } catch (RuntimeException ignored) { }
        String fallback = uri.getLastPathSegment();
        return fallback == null || fallback.isEmpty() ? "document" : fallback;
    }

    void close() {
        completePending(PlatformEnvelope.failure("activity-destroyed"));
        keyfiles.closeAll();
    }
}
