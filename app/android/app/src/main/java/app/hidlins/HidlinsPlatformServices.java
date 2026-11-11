package app.hidlins;

import android.app.Activity;
import android.content.ContentResolver;
import android.content.res.AssetFileDescriptor;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.CancellationSignal;
import android.os.Handler;
import android.os.Looper;
import android.os.OperationCanceledException;
import android.provider.OpenableColumns;
import io.flutter.plugin.common.BinaryMessenger;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.MethodChannel;
import java.io.File;
import java.io.FilterInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.ref.WeakReference;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

/** Narrow Android mechanism adapter. Vault and lock policy remain in Rust. */
final class HidlinsPlatformServices {
    private static final int PICK_VAULT = 0x4849;
    private static final int PICK_KEYFILE = 0x484b;
    private final MainActivity activity;
    private final AppStorage storage;
    private final ClipboardController clipboard;
    private final KeyfileReferenceStore keyfiles;
    private final SnapshotCoverController cover;
    private final LocalDiscoveryController localDiscovery;
    private MethodChannel lifecycle;
    private String lifecycleState = "inactive";
    private String lifecycleObservationToken = UUID.randomUUID().toString();
    private MethodChannel.Result pendingPicker;
    private int pendingRequest;
    private final ExecutorService providerWorker = Executors.newSingleThreadExecutor();
    private DocumentWork activeWork;
    private boolean closed;

    HidlinsPlatformServices(MainActivity activity, SnapshotCoverController cover) {
        this.activity = activity;
        this.cover = cover;
        storage = new AppStorage(activity.getApplicationContext());
        clipboard = new ClipboardController(activity);
        keyfiles = new KeyfileReferenceStore(activity.getApplicationContext());
        localDiscovery = new LocalDiscoveryController(activity);
    }

    void register(BinaryMessenger messenger) {
        lifecycle = new MethodChannel(messenger, "app.hidlins/lifecycle");
        lifecycle.setMethodCallHandler(this::handleLifecycle);
        channel(messenger, "app.hidlins/paths", this::handlePaths);
        channel(messenger, "app.hidlins/clipboard", this::handleClipboard);
        channel(messenger, "app.hidlins/vault_import", this::handleVaultImport);
        channel(messenger, "app.hidlins/keyfile", this::handleKeyfile);
        channel(messenger, "app.hidlins/local_discovery", this::handleLocalDiscovery);
    }

    private static void channel(
            BinaryMessenger messenger, String name, MethodChannel.MethodCallHandler handler) {
        new MethodChannel(messenger, name).setMethodCallHandler(handler);
    }

    void reportLifecycle(String state) {
        // Observe before publishing or changing the cover. A reply for any
        // preceding observation must never remove this observation's cover.
        lifecycleState = state;
        lifecycleObservationToken = UUID.randomUUID().toString();
        if (!"resumed".equals(state)) cover.show(activity);
        if (!"resumed".equals(state)) localDiscovery.stop();
        sendLifecycleObservation();
    }

    private void sendLifecycleObservation() {
        if (lifecycle != null) {
            Map<String, Object> arguments = new HashMap<>();
            arguments.put("state", lifecycleState);
            arguments.put("observationToken", lifecycleObservationToken);
            lifecycle.invokeMethod("reportState", arguments);
        }
    }

    private void handleLocalDiscovery(MethodCall call, MethodChannel.Result result) {
        switch (call.method) {
            case "permissionStatus" -> result.success(
                    PlatformEnvelope.success(localDiscovery.permissionStatus()).envelope());
            case "discover" -> {
                Map<?, ?> arguments = call.arguments instanceof Map<?, ?> map ? map : null;
                Object kind = arguments == null ? null : arguments.get("kind");
                Object timeout = arguments == null ? null : arguments.get("timeoutMs");
                if (!(kind instanceof String value) || !(timeout instanceof Integer milliseconds)) {
                    result.success(PlatformEnvelope.failure("invalid-discovery-request").envelope());
                } else {
                    localDiscovery.discover(value, milliseconds, result);
                }
            }
            case "openSettings" -> localDiscovery.openSettings(result);
            case "stop" -> {
                localDiscovery.stop();
                result.success(PlatformEnvelope.success(null).envelope());
            }
            default -> result.success(PlatformEnvelope.unsupported().envelope());
        }
    }

    void onRequestPermissionsResult(int requestCode, int[] grantResults) {
        localDiscovery.onPermissionResult(requestCode, grantResults);
    }

    private void handleLifecycle(MethodCall call, MethodChannel.Result result) {
        Map<?, ?> arguments = call.arguments instanceof Map<?, ?> map ? map : null;
        if ("reportState".equals(call.method) && arguments != null
                && Boolean.TRUE.equals(arguments.get("requestObservation"))) {
            // Startup replay keeps the current token. It cannot remove a cover.
            sendLifecycleObservation();
            result.success(PlatformEnvelope.success(null).envelope());
            return;
        }
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
        Object token = arguments.get("observationToken");
        if (!lifecycleObservationToken.equals(token) || !lifecycleState.equals(state)) {
            sendLifecycleObservation();
            result.success(PlatformEnvelope.stale().envelope());
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
        if (closed || pendingPicker != null) return false;
        pendingPicker = result;
        pendingRequest = request;
        return true;
    }

    boolean onActivityResult(int requestCode, int resultCode, Intent data) {
        if (pendingPicker == null || requestCode != pendingRequest) return false;
        if (activeWork != null) return true;
        if (resultCode != Activity.RESULT_OK || data == null || data.getData() == null) {
            completePending(PlatformEnvelope.canceled());
            return true;
        }
        DocumentWork work = new DocumentWork(this, requestCode, data.getData());
        activeWork = work;
        try {
            providerWorker.execute(work);
        } catch (RuntimeException error) {
            activeWork = null;
            work.cancel();
            completePending(PlatformEnvelope.failure("document-access-failed"));
        }
        return true;
    }

    private void completeWork(DocumentWork work, DocumentResult outcome) {
        if (closed || activeWork != work || pendingPicker == null || work.canceled.get()) {
            work.decision.countDown();
            return;
        }
        activeWork = null;
        MethodChannel.Result result = takePending();
        try {
            result.success(outcome.envelope.envelope());
            work.delivered.set(true);
        } catch (RuntimeException ignored) {
            // A destroyed engine cannot accept ownership of an imported file.
        } finally {
            work.decision.countDown();
        }
    }

    private static final class DocumentResult {
        final PlatformEnvelope.Result envelope;
        final File imported;
        final KeyfileReferenceStore.PendingReference reference;

        DocumentResult(PlatformEnvelope.Result envelope, File imported,
                       KeyfileReferenceStore.PendingReference reference) {
            this.envelope = envelope;
            this.imported = imported;
            this.reference = reference;
        }

        void complete(boolean delivered) {
            if (reference != null) reference.complete(delivered);
            if (!delivered && imported != null && !imported.delete()) imported.deleteOnExit();
        }
    }

    /** Contains application context resources and a weak owner, never a result or activity. */
    private static final class DocumentWork implements Runnable {
        final WeakReference<HidlinsPlatformServices> owner;
        final ContentResolver resolver;
        final AppStorage storage;
        final KeyfileReferenceStore keyfiles;
        final Handler main = new Handler(Looper.getMainLooper());
        final int request;
        final Uri uri;
        final CancellationSignal cancellation = new CancellationSignal();
        final AtomicBoolean canceled = new AtomicBoolean();
        final AtomicBoolean delivered = new AtomicBoolean();
        final CountDownLatch decision = new CountDownLatch(1);
        final AtomicReference<InputStream> input = new AtomicReference<>();

        DocumentWork(HidlinsPlatformServices services, int request, Uri uri) {
            owner = new WeakReference<>(services);
            resolver = services.activity.getApplicationContext().getContentResolver();
            storage = services.storage;
            keyfiles = services.keyfiles;
            this.request = request;
            this.uri = uri;
        }

        void cancel() {
            if (!canceled.compareAndSet(false, true)) return;
            decision.countDown();
            // Provider cancellation itself may block in Binder. Neither it nor
            // stream closure may hold up activity destruction on the main loop.
            Thread canceler = new Thread(() -> {
                InputStream opened = input.get();
                if (opened != null) {
                    try { opened.close(); } catch (IOException | RuntimeException ignored) { }
                }
                try { cancellation.cancel(); } catch (RuntimeException ignored) { }
            }, "hidlins-document-cancel");
            canceler.setDaemon(true);
            canceler.start();
        }

        void checkCanceled() {
            if (canceled.get()) throw new OperationCanceledException();
        }

        @Override public void run() {
            DocumentResult outcome;
            try {
                outcome = readDocument();
            } catch (SecurityException error) {
                outcome = new DocumentResult(PlatformEnvelope.denied(), null, null);
            } catch (IOException | RuntimeException error) {
                outcome = new DocumentResult(PlatformEnvelope.failure("document-access-failed"), null, null);
            }
            try {
                if (!canceled.get()) {
                    DocumentResult completed = outcome;
                    if (main.post(() -> {
                        HidlinsPlatformServices services = owner.get();
                        if (services == null) decision.countDown();
                        else services.completeWork(this, completed);
                    })) decision.await();
                }
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
            } finally {
                outcome.complete(delivered.get());
            }
        }

        private DocumentResult readDocument() throws IOException {
            checkCanceled();
            String name = displayName();
            checkCanceled();
            if (request == PICK_KEYFILE) {
                KeyfileReferenceStore.PendingReference saved = keyfiles.savePending(uri, name);
                return new DocumentResult(saved.result, null, saved);
            }
            File imported = null;
            try {
                try (AssetFileDescriptor descriptor = resolver.openAssetFileDescriptor(uri, "r", cancellation)) {
                    if (descriptor == null) return new DocumentResult(PlatformEnvelope.stale(), null, null);
                    try (InputStream stream = descriptor.createInputStream()) {
                        input.set(stream);
                        checkCanceled();
                        InputStream checked = new FilterInputStream(stream) {
                            @Override public int read(byte[] bytes, int offset, int length) throws IOException {
                                checkCanceled();
                                int count = super.read(bytes, offset, length);
                                checkCanceled();
                                return count;
                            }
                        };
                        imported = storage.importVault(checked, name);
                        Map<String, Object> value = new HashMap<>();
                        value.put("sourceReference", imported.getAbsolutePath());
                        value.put("displayName", imported.getName().replaceFirst("\\.kdbx$", ""));
                        return new DocumentResult(PlatformEnvelope.success(value), imported, null);
                    } finally {
                        input.set(null);
                    }
                }
            } catch (IOException | RuntimeException error) {
                if (imported != null && !imported.delete()) imported.deleteOnExit();
                throw error;
            }
        }

        private String displayName() {
            try (Cursor cursor = resolver.query(uri, new String[]{OpenableColumns.DISPLAY_NAME},
                    null, null, null, cancellation)) {
                if (cursor != null && cursor.moveToFirst()) {
                    String name = cursor.getString(0);
                    if (name != null && !name.isEmpty()) return name;
                }
            } catch (RuntimeException ignored) { }
            String fallback = uri.getLastPathSegment();
            return fallback == null || fallback.isEmpty() ? "document" : fallback;
        }
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

    void close() {
        if (closed) return;
        closed = true;
        DocumentWork work = activeWork;
        activeWork = null;
        if (work != null) work.cancel();
        providerWorker.shutdownNow();
        try {
            completePending(PlatformEnvelope.failure("activity-destroyed"));
        } finally {
            localDiscovery.stop();
            keyfiles.closeAll();
        }
    }
}
