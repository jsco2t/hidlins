package app.hidlins;

import static app.hidlins.DeviceAssertions.assertArrayEquals;
import static app.hidlins.DeviceAssertions.assertEquals;
import static app.hidlins.DeviceAssertions.assertFalse;
import static app.hidlins.DeviceAssertions.assertNotNull;
import static app.hidlins.DeviceAssertions.assertTrue;

import android.app.Instrumentation;
import android.os.Bundle;
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

/** Device tests for Hidlins' Android security and storage mechanisms. */
public final class HidlinsAndroidTest extends Instrumentation {
    private int completed;
    private int failures;

    @Override
    public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override
    public void onStart() {
        runCase("secureWindowAndSnapshotCoverAreInstalled", this::secureWindowAndSnapshotCoverAreInstalled);
        runCase("clipboardClearIsOwnershipCheckedAndSensitive", this::clipboardClearIsOwnershipCheckedAndSensitive);
        runCase("vaultImportIsAtomicAndAppOwned", this::vaultImportIsAtomicAndAppOwned);
        runCase("missingAndInvalidKeyfileReferencesFailClosed", this::missingAndInvalidKeyfileReferencesFailClosed);
        runCase("appBackupIsDisabled", this::appBackupIsDisabled);
        runCase("shippingApplicationCanReachConfiguredSyncStorage", this::shippingApplicationCanReachConfiguredSyncStorage);
        runCase("localDiscoveryPayloadIsBoundedAndContainsOnlyRoutes", this::localDiscoveryPayloadIsBoundedAndContainsOnlyRoutes);
        runCase("discoveryAttemptGenerationRejectsStoppedAndDuplicateCallbacks", this::discoveryAttemptGenerationRejectsStoppedAndDuplicateCallbacks);
        runCase("scenarioRegistrarAcceptsOnlyBoundedPortAndKnownServiceKind", this::scenarioRegistrarAcceptsOnlyBoundedPortAndKnownServiceKind);
        runCase("pickerCancellationAndActivityDestructionCompletePendingResults", this::pickerCancellationAndActivityDestructionCompletePendingResults);
        runCase("vaultImportReportsProviderPermissionDenial", this::vaultImportReportsProviderPermissionDenial);
        runCase("lifecycleRejectsStaleAfterPause", () -> LifecycleAckRegressions.staleAfterPause(this));
        runCase("lifecycleRejectsResumePauseResumeABA", () -> LifecycleAckRegressions.staleAcrossResumeABA(this));
        runCase("lifecycleRejectsUnnumberedAcknowledgement", () -> LifecycleAckRegressions.unnumberedCannotHide(this));
        runCase("lifecycleAcceptsCurrentAcknowledgement", () -> LifecycleAckRegressions.currentAcknowledgementHides(this));
        runCase("lifecycleBootstrapReplaysCurrentObservation", () -> LifecycleAckRegressions.bootstrapReplaysCurrent(this));
        runCase("safVaultQueryKeepsMainResponsive", () -> SafWorkerRegressions.vaultStageDoesNotBlockMain(this, "query"));
        runCase("safVaultOpenKeepsMainResponsive", () -> SafWorkerRegressions.vaultStageDoesNotBlockMain(this, "open"));
        runCase("safVaultReadKeepsMainResponsive", () -> SafWorkerRegressions.vaultStageDoesNotBlockMain(this, "read"));
        runCase("safKeyfileQueryKeepsMainResponsive", () -> SafWorkerRegressions.keyfileQueryDoesNotBlockMain(this));
        runCase("safDestroyDuringVaultQueryCompletesPromptly", () -> SafWorkerRegressions.destroyDuringProviderWork(this, "query", false));
        runCase("safDestroyDuringVaultOpenClosesLateReader", () -> SafWorkerRegressions.destroyDuringProviderWork(this, "open", false));
        runCase("safDestroyDuringVaultReadCleansPartialCopy", () -> SafWorkerRegressions.destroyDuringProviderWork(this, "read", false));
        runCase("safDestroyDuringKeyfileQueryRollsBack", () -> SafWorkerRegressions.destroyDuringProviderWork(this, "query", true));
        runCase("safDeliveredImportSurvivesDestroy", () -> SafWorkerRegressions.deliveredImportSurvivesDestruction(this));
        runCase("safDisposeAfterInstallCleansUndeliveredFile", () -> SafWorkerRegressions.disposalAfterInstallBeforeDelivery(this));
        runCase("safInputClosesOnSetupFailure", () -> SafStorageRegressions.closesInputOnSetupFailure(this));
        runCase("safPartialReadFailureCleansStaging", () -> SafStorageRegressions.cleansPartialReadFailure(this));
        runCase("safOverlappingActivitiesNeverOverwriteImports", () -> SafStorageRegressions.overlappingImportsNeverOverwrite(this));
        runCase("safFailedKeyfileSavePreservesExistingGrant", () -> SafKeyfileRegressions.failedSavePreservesEarlierGrant(this));
        runCase("safFailedNewKeyfileSaveLeavesNoGrant", () -> SafKeyfileRegressions.failedNewSaveLeavesNoGrant(this));
        Bundle result = new Bundle();
        result.putString("stream", "HIDLINS_DEVICE_TESTS " + completed + " passed, " + failures + " failed\n");
        finish(failures == 0 && completed == 31 ? -1 : 0, result);
    }

    private void runCase(String name, DeviceCase test) {
        try {
            test.run();
            completed += 1;
            report("PASS " + name);
        } catch (Exception | AssertionError error) {
            failures += 1;
            // Never print assertion operands: tests exercise secret buffers.
            report("FAIL " + name + " (" + error.getClass().getSimpleName() + ", "
                    + nativeFailureLocation(error) + ")");
        } finally {
            MainActivity activity = MainActivity.currentForTest();
            if (activity != null && !activity.isFinishing()) {
                try {
                    runOnMain(activity::finish);
                } catch (AssertionError error) {
                    failures += 1;
                    report("FAIL " + name + " cleanup");
                }
            }
        }
    }

    private void report(String message) {
        Bundle status = new Bundle();
        status.putString("stream", message + "\n");
        sendStatus(0, status);
    }

    private static String nativeFailureLocation(Throwable error) {
        Throwable cause = error;
        while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
        for (StackTraceElement frame : cause.getStackTrace()) {
            String name = frame.getClassName();
            for (String fixture : new String[]{"SafWorkerRegressions", "SafStorageRegressions",
                    "SafKeyfileRegressions", "HidlinsControlledDocumentProvider"}) {
                if (name.equals("app.hidlins." + fixture)
                        || name.startsWith("app.hidlins." + fixture + "$")) {
                    return fixture + ".java:" + frame.getLineNumber();
                }
            }
        }
        return DeviceAssertions.failureLocation(error);
    }

    @FunctionalInterface
    private interface DeviceCase {
        void run() throws Exception;
    }

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

    public void vaultImportIsAtomicAndAppOwned() throws Exception {
        Context context = context();
        File root = new File(context.getCacheDir(), "instrumented-import");
        AppStorage storage = new AppStorage(context, root);
        byte[] fixture = "kdbx-fixture".getBytes(StandardCharsets.UTF_8);
        File imported = storage.importVault(new ByteArrayInputStream(fixture), "personal.kdbx");
        assertEquals("personal.kdbx", imported.getName());
        assertArrayEquals(fixture, Files.readAllBytes(imported.toPath()));
        File[] leftovers = new File(root, "Vaults").listFiles((dir, name) -> name.startsWith(".import-"));
        assertNotNull(leftovers);
        assertEquals(0, leftovers.length);
    }

    public void missingAndInvalidKeyfileReferencesFailClosed() {
        Context context = context();
        KeyfileReferenceStore store = new KeyfileReferenceStore(context);
        assertEquals(PlatformEnvelope.Status.STALE, store.resolve("missing-reference").status());
        assertEquals(PlatformEnvelope.Status.FAILURE,
                store.save(Uri.parse("http://invalid"), "bad.key").status());
    }

    public void appBackupIsDisabled() throws Exception {
        Context context = context();
        int flags = context.getPackageManager().getApplicationInfo(context.getPackageName(), 0).flags;
        assertFalse((flags & android.content.pm.ApplicationInfo.FLAG_ALLOW_BACKUP) != 0);
    }

    public void shippingApplicationCanReachConfiguredSyncStorage() {
        Context context = context();
        assertEquals(
                PackageManager.PERMISSION_GRANTED,
                context.getPackageManager().checkPermission(
                        android.Manifest.permission.INTERNET,
                        context.getPackageName()));
    }

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

    // LNS-ANDROID-012: stopped discovery callbacks cannot acquire a restarted attempt.
    public void discoveryAttemptGenerationRejectsStoppedAndDuplicateCallbacks() {
        DiscoveryAttemptTracker attempts = new DiscoveryAttemptTracker();
        long first = attempts.begin();
        assertTrue(attempts.isActive(first));

        attempts.invalidate();
        long second = attempts.begin();
        assertFalse(attempts.isActive(first));
        assertTrue(attempts.isActive(second));
        assertFalse(attempts.complete(first));
        assertTrue(attempts.isActive(second));

        assertTrue(attempts.complete(second));
        assertFalse(attempts.complete(second));
        assertFalse(attempts.isActive(second));
    }

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

    public void pickerCancellationAndActivityDestructionCompletePendingResults() {
        MainActivity activity = launchActivity();
        HidlinsPlatformServices services =
                new HidlinsPlatformServices(activity, new SnapshotCoverController());

        RecordingResult canceled = new RecordingResult();
        runOnMain(() -> {
            assertTrue(services.beginPicker(71, canceled));
            assertTrue(services.onActivityResult(71, android.app.Activity.RESULT_CANCELED, null));
        });
        assertEquals("canceled", canceled.status());

        RecordingResult destroyed = new RecordingResult();
        runOnMain(() -> {
            assertTrue(services.beginPicker(72, destroyed));
            services.close();
        });
        assertEquals("failure", destroyed.status());
        assertEquals("activity-destroyed", destroyed.code());
        runOnMain(activity::finish);
    }

    public void vaultImportReportsProviderPermissionDenial() {
        MainActivity activity = launchActivity();
        HidlinsPlatformServices services =
                new HidlinsPlatformServices(activity, new SnapshotCoverController());
        RecordingResult denied = new RecordingResult();
        runOnMain(() -> assertTrue(services.beginPicker(73, denied)));
        Intent result = new Intent().setData(
                Uri.parse("content://app.hidlins.test.denied/vault.kdbx"));
        runOnMain(() -> assertTrue(services.onActivityResult(73, android.app.Activity.RESULT_OK, result)));
        assertEquals("denied", denied.status());
        runOnMain(services::close);
        runOnMain(activity::finish);
    }

    private Context context() {
        Context target = getTargetContext();
        assertNotNull(target);
        return target;
    }

    private MainActivity launchActivity() {
        Context target = context();
        MainActivity activity = (MainActivity) startActivitySync(
                new Intent(target, MainActivity.class)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        waitForIdleSync();
        // Clipboard access requires a focused window, not merely onCreate.
        // Always wait for the activity returned by this launch, never a stale
        // process-wide reference from the preceding case.
        for (int attempt = 0; attempt < 1500; attempt += 1) {
            if (activity.hasWindowFocus()) return activity;
            try {
                Thread.sleep(20);
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError("activity wait interrupted", error);
            }
        }
        throw new AssertionError("activity did not gain focus");
    }

    private static void runOnMain(Runnable action) {
        CountDownLatch completed = new CountDownLatch(1);
        java.util.concurrent.atomic.AtomicReference<Throwable> failure =
                new java.util.concurrent.atomic.AtomicReference<>();
        new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> {
            try {
                action.run();
            } catch (Throwable error) {
                failure.set(error);
            } finally {
                completed.countDown();
            }
        });
        try {
            assertTrue(completed.await(2, TimeUnit.SECONDS));
            if (failure.get() != null) {
                throw new AssertionError("main-thread action failed", failure.get());
            }
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new AssertionError("main-thread action interrupted", error);
        }
    }

    private static final class RecordingResult implements MethodChannel.Result {
        private volatile Object value;
        private final CountDownLatch replied = new CountDownLatch(1);

        @Override
        public void success(Object result) {
            value = result;
            replied.countDown();
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
            try {
                assertTrue(replied.await(5, TimeUnit.SECONDS));
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError("fixture reply wait interrupted", error);
            }
            assertTrue(value instanceof java.util.Map<?, ?>);
            return (String) ((java.util.Map<?, ?>) value).get("status");
        }

        String code() {
            assertTrue(value instanceof java.util.Map<?, ?>);
            return (String) ((java.util.Map<?, ?>) value).get("code");
        }
    }
}
