package app.hidlins;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.view.WindowManager;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import io.flutter.plugin.common.MethodChannel;
import org.junit.Test;

/** Device tests for Hidlins' Android security and storage mechanisms. */
public final class HidlinsAndroidTest {
    @Test
    public void secureWindowAndSnapshotCoverAreInstalled() {
        MainActivity activity = launchActivity();
        assertTrue((activity.getWindow().getAttributes().flags
                & WindowManager.LayoutParams.FLAG_SECURE) != 0);
        boolean[] visibleSynchronously = {false};
        runOnMain(() -> {
            activity.showSecurityCoverForTest();
            visibleSynchronously[0] = activity.isSecurityCoverVisibleForTest();
        });
        assertTrue(visibleSynchronously[0]);
        runOnMain(activity::finish);
    }

    @Test
    public void clipboardClearIsOwnershipCheckedAndSensitive() {
        Context context = context();
        MainActivity activity = launchActivity();
        ClipboardController controller = new ClipboardController(context);
        byte[] secret = "hidlins-clipboard-canary".getBytes(StandardCharsets.UTF_8);
        assertTrue(controller.write(secret, 30));
        assertArrayEquals(new byte[secret.length], secret);

        ClipboardManager clipboard =
                (ClipboardManager) context.getSystemService(Context.CLIPBOARD_SERVICE);
        assertNotNull(clipboard.getPrimaryClipDescription());
        if (Build.VERSION.SDK_INT >= 33) {
            assertTrue(clipboard.getPrimaryClipDescription().getExtras()
                    .getBoolean("android.content.extra.IS_SENSITIVE"));
        }
        clipboard.setPrimaryClip(ClipData.newPlainText("foreign", "leave-me"));
        controller.clearOwnedNowForTest();
        assertEquals("leave-me", clipboard.getPrimaryClip().getItemAt(0).coerceToText(context));

        byte[] second = "owned".getBytes(StandardCharsets.UTF_8);
        assertTrue(controller.write(second, 30));
        controller.clearOwnedNowForTest();
        assertTrue(!clipboard.hasPrimaryClip()
                || clipboard.getPrimaryClip().getItemCount() == 0
                || clipboard.getPrimaryClip().getItemAt(0).coerceToText(context).length() == 0);
        runOnMain(activity::finish);
    }

    @Test
    public void vaultImportIsAtomicAndAppOwned() throws Exception {
        Context context = context();
        File root = new File(context.getCacheDir(), "instrumented-import");
        AppStorage storage = new AppStorage(context, root);
        byte[] fixture = "kdbx-fixture".getBytes(StandardCharsets.UTF_8);
        File imported = storage.importVault(new ByteArrayInputStream(fixture), "personal.kdbx");
        assertEquals("personal.kdbx", imported.getName());
        assertArrayEquals(fixture, Files.readAllBytes(imported.toPath()));
        File[] leftovers = root.listFiles((dir, name) -> name.startsWith(".import-"));
        assertNotNull(leftovers);
        assertEquals(0, leftovers.length);
    }

    @Test
    public void missingAndInvalidKeyfileReferencesFailClosed() {
        Context context = context();
        KeyfileReferenceStore store = new KeyfileReferenceStore(context);
        assertEquals(PlatformEnvelope.Status.STALE, store.resolve("missing-reference").status());
        assertEquals(PlatformEnvelope.Status.FAILURE,
                store.save(Uri.parse("http://invalid"), "bad.key").status());
    }

    @Test
    public void appBackupIsDisabled() throws Exception {
        Context context = context();
        int flags = context.getPackageManager().getApplicationInfo(context.getPackageName(), 0).flags;
        assertFalse((flags & android.content.pm.ApplicationInfo.FLAG_ALLOW_BACKUP) != 0);
    }

    @Test
    public void shippingApplicationCanReachConfiguredSyncStorage() {
        Context context = context();
        assertEquals(
                PackageManager.PERMISSION_GRANTED,
                context.getPackageManager().checkPermission(
                        android.Manifest.permission.INTERNET,
                        context.getPackageName()));
    }

    @Test
    public void localDiscoveryPayloadIsBoundedAndContainsOnlyRoutes() {
        List<Map<String, Object>> routes = new ArrayList<>();
        for (int index = 0; index < 20; index += 1) {
            Map<String, Object> route = new HashMap<>();
            route.put("address", "192.168.1." + index);
            route.put("port", 42_873);
            route.put("scopeId", 0);
            route.put("vault", "must-not-cross");
            routes.add(route);
        }
        Map<String, Object> payload = LocalDiscoveryController.boundedPayload("granted", routes);
        assertEquals("granted", payload.get("permission"));
        assertEquals(LocalDiscoveryController.MAX_CANDIDATES,
                ((List<?>) payload.get("candidates")).size());
        assertFalse(payload.toString().contains("vault"));
        assertFalse(payload.toString().contains("key"));
        assertFalse(payload.toString().contains("sas"));
    }

    @Test
    public void scenarioRegistrarAcceptsOnlyBoundedPortAndKnownServiceKind() {
        assertTrue(ScenarioNsdRegistrarActivity.validPort(1));
        assertTrue(ScenarioNsdRegistrarActivity.validPort(65_535));
        assertFalse(ScenarioNsdRegistrarActivity.validPort(0));
        assertFalse(ScenarioNsdRegistrarActivity.validPort(65_536));
        assertEquals(
                "_hidlins-pair._tcp.",
                ScenarioNsdRegistrarActivity.serviceType("pairing"));
        assertEquals(
                "_hidlins-sync._tcp.",
                ScenarioNsdRegistrarActivity.serviceType("trusted"));
        assertEquals(null, ScenarioNsdRegistrarActivity.serviceType("other"));
    }

    @Test
    public void pickerCancellationAndActivityDestructionCompletePendingResults() {
        MainActivity activity = launchActivity();
        HidlinsPlatformServices services =
                new HidlinsPlatformServices(activity, new SnapshotCoverController());

        RecordingResult canceled = new RecordingResult();
        assertTrue(services.beginPicker(71, canceled));
        assertTrue(services.onActivityResult(71, android.app.Activity.RESULT_CANCELED, null));
        assertEquals("canceled", canceled.status());

        RecordingResult destroyed = new RecordingResult();
        assertTrue(services.beginPicker(72, destroyed));
        services.close();
        assertEquals("failure", destroyed.status());
        assertEquals("activity-destroyed", destroyed.code());
        runOnMain(activity::finish);
    }

    @Test
    public void vaultImportReportsProviderPermissionDenial() {
        MainActivity activity = launchActivity();
        HidlinsPlatformServices services =
                new HidlinsPlatformServices(activity, new SnapshotCoverController());
        RecordingResult denied = new RecordingResult();
        assertTrue(services.beginPicker(73, denied));
        Intent result = new Intent().setData(
                Uri.parse("content://app.hidlins.test.denied/vault.kdbx"));
        assertTrue(services.onActivityResult(73, android.app.Activity.RESULT_OK, result));
        assertEquals("denied", denied.status());
        services.close();
        runOnMain(activity::finish);
    }

    private static Context context() {
        try {
            Class<?> registry = Class.forName(
                    "androidx.test.platform.app.InstrumentationRegistry");
            Object value = registry.getMethod("getInstrumentation").invoke(null);
            assertTrue(value instanceof android.app.Instrumentation);
            Context context = ((android.app.Instrumentation) value).getTargetContext();
            assertNotNull(context);
            return context;
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("Android instrumentation registry is unavailable", error);
        }
    }

    private static MainActivity awaitActivity() throws AssertionError {
        for (int attempt = 0; attempt < 250; attempt += 1) {
            MainActivity activity = MainActivity.currentForTest();
            if (activity != null && !activity.isFinishing() && activity.hasWindowFocus()) {
                return activity;
            }
            try {
                Thread.sleep(20);
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError("activity wait interrupted", error);
            }
        }
        throw new AssertionError("activity did not start");
    }

    private static MainActivity launchActivity() {
        Context target = context();
        target.startActivity(new Intent(target, MainActivity.class)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        return awaitActivity();
    }

    private static void runOnMain(Runnable action) {
        CountDownLatch completed = new CountDownLatch(1);
        new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> {
            try {
                action.run();
            } finally {
                completed.countDown();
            }
        });
        try {
            assertTrue(completed.await(2, TimeUnit.SECONDS));
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new AssertionError("main-thread action interrupted", error);
        }
    }

    private static final class RecordingResult implements MethodChannel.Result {
        private Object value;

        @Override
        public void success(Object result) {
            value = result;
        }

        @Override
        public void error(String code, String message, Object details) {
            throw new AssertionError("unexpected platform error: " + code);
        }

        @Override
        public void notImplemented() {
            throw new AssertionError("unexpected notImplemented result");
        }

        String status() {
            assertTrue(value instanceof java.util.Map<?, ?>);
            return (String) ((java.util.Map<?, ?>) value).get("status");
        }

        String code() {
            assertTrue(value instanceof java.util.Map<?, ?>);
            return (String) ((java.util.Map<?, ?>) value).get("code");
        }
    }
}
