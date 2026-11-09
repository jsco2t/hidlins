package app.hidlins;

import static app.hidlins.DeviceAssertions.assertArrayEquals;
import static app.hidlins.DeviceAssertions.assertEquals;
import static app.hidlins.DeviceAssertions.assertFalse;
import static app.hidlins.DeviceAssertions.assertNotNull;
import static app.hidlins.DeviceAssertions.assertTrue;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.UriPermission;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import io.flutter.plugin.common.MethodChannel;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.util.HashSet;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/** Actual activity-owned controller plus a separately hosted provider. */
final class SafWorkerRegressions {
    private static final int PICK_VAULT = 0x4849;
    private static final int PICK_KEYFILE = 0x484b;
    private static final Uri PROVIDER = Uri.parse("content://" + HidlinsControlledDocumentProvider.AUTHORITY);

    private static final class Result implements MethodChannel.Result {
        final CountDownLatch replied = new CountDownLatch(1);
        final AtomicInteger calls = new AtomicInteger();
        final AtomicBoolean onMain = new AtomicBoolean();
        final AtomicReference<Object> value = new AtomicReference<>();

        @Override public void success(Object envelope) {
            value.set(envelope);
            onMain.set(Looper.myLooper() == Looper.getMainLooper());
            calls.incrementAndGet();
            replied.countDown();
        }
        @Override public void error(String code, String message, Object details) { success("channel-error"); }
        @Override public void notImplemented() { success("not-implemented"); }
        Map<?, ?> envelope() { assertTrue(value.get() instanceof Map<?, ?>); return (Map<?, ?>) value.get(); }
        Map<?, ?> body() { Object body = envelope().get("value"); assertTrue(body instanceof Map<?, ?>); return (Map<?, ?>) body; }
    }

    private static final class MainTask {
        final CountDownLatch done = new CountDownLatch(1);
        final AtomicReference<Throwable> failure = new AtomicReference<>();
        MainTask(Runnable action) {
            new Handler(Looper.getMainLooper()).post(() -> {
                try { action.run(); } catch (Throwable error) { failure.set(error); }
                finally { done.countDown(); }
            });
        }
        void await() throws Exception {
            assertTrue(done.await(5, TimeUnit.SECONDS));
            if (failure.get() != null) throw new AssertionError("fixture main action failed", failure.get());
        }
    }

    private static void onMain(Runnable action) throws Exception { new MainTask(action).await(); }

    private static final class Fixture implements AutoCloseable {
        final MainActivity activity;
        final HidlinsPlatformServices services;
        final Context context;
        final ContentResolver resolver;
        final String id = UUID.randomUUID().toString();
        final Uri uri = PROVIDER.buildUpon().appendPath(id).build();
        final Result result = new Result();
        final Set<String> beforeFiles = new HashSet<>();
        final Map<String, Object> beforeReferences = new HashMap<>();
        final Set<String> beforeGrants = new HashSet<>();
        final boolean keyfile;
        final String phase;
        MainTask dispatch;

        Fixture(Instrumentation instrumentation, String phase, boolean keyfile) throws Exception {
            this(instrumentation, phase, keyfile, false);
        }

        Fixture(Instrumentation instrumentation, String phase, boolean keyfile, boolean observeReaderClose)
                throws Exception {
            this.keyfile = keyfile;
            this.phase = phase;
            context = instrumentation.getTargetContext();
            resolver = context.getContentResolver();
            activity = (MainActivity) instrumentation.startActivitySync(new Intent(context, MainActivity.class)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            instrumentation.waitForIdleSync();
            Field field = MainActivity.class.getDeclaredField("platformServices");
            field.setAccessible(true);
            services = (HidlinsPlatformServices) field.get(activity);
            assertNotNull(services);
            File[] existing = vaults().listFiles();
            if (existing != null) for (File file : existing) beforeFiles.add(file.getCanonicalPath());
            Bundle arguments = new Bundle();
            arguments.putString("phase", phase);
            arguments.putBoolean("observeReaderClose", observeReaderClose);
            Bundle configured = resolver.call(PROVIDER, "configure", id, arguments);
            assertNotNull(configured);
            assertTrue(configured.getBoolean("configured"));
            if (keyfile) resolver.call(PROVIDER, "grant", id, null);
            beforeReferences.putAll(context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE).getAll());
            for (UriPermission permission : resolver.getPersistedUriPermissions()) {
                if (permission.isReadPermission()) beforeGrants.add(permission.getUri().toString());
            }
        }

        File vaults() { return new File(context.getFilesDir(), "hidlins/Vaults"); }
        Bundle state() {
            Bundle state = resolver.call(PROVIDER, "state", id, null);
            assertNotNull(state);
            return state;
        }
        void begin() throws Exception {
            int request = keyfile ? PICK_KEYFILE : PICK_VAULT;
            onMain(() -> assertTrue(services.beginPicker(request, result)));
            dispatch = new MainTask(() -> assertTrue(services.onActivityResult(request, Activity.RESULT_OK,
                    new Intent().setData(uri))));
            if (!phase.equals("none")) {
                long deadline = SystemClock.elapsedRealtime() + 5000;
                while (!state().getBoolean("entered") && SystemClock.elapsedRealtime() < deadline) SystemClock.sleep(20);
                assertTrue(state().getBoolean("entered"));
                assertFalse(state().getBoolean("timedOut"));
                if (phase.equals("read")) {
                    // Provider writer entry precedes app open/read. Require
                    // actual partial app copying before canceling that read.
                    long copiedDeadline = SystemClock.elapsedRealtime() + 3000;
                    boolean copied;
                    do {
                        copied = false;
                        File[] files = vaults().listFiles();
                        if (files != null) for (File file : files) {
                            if (file.getName().startsWith(".import-") && file.length() > 0
                                    && !beforeFiles.contains(file.getCanonicalPath())) copied = true;
                        }
                        if (!copied) SystemClock.sleep(20);
                    } while (!copied && SystemClock.elapsedRealtime() < copiedDeadline);
                    assertTrue(copied);
                }
            }
        }
        void release() { resolver.call(PROVIDER, "release", id, null); }
        void complete() throws Exception {
            if (dispatch != null) dispatch.await();
            assertTrue(result.replied.await(5, TimeUnit.SECONDS));
            assertFalse(state().getBoolean("timedOut"));
        }
        void verifySuccess() throws Exception {
            assertEquals(1, result.calls.get());
            assertTrue(result.onMain.get());
            assertEquals("success", result.envelope().get("status"));
            if (!keyfile) {
                File imported = new File((String) result.body().get("sourceReference"));
                assertEquals(vaults().getCanonicalPath(), imported.getParentFile().getCanonicalPath());
                assertFalse(beforeFiles.contains(imported.getCanonicalPath()));
                assertArrayEquals(HidlinsControlledDocumentProvider.CONTENT, Files.readAllBytes(imported.toPath()));
                assertEquals(imported.getName().replaceFirst("\\.kdbx$", ""), result.body().get("displayName"));
            } else {
                Object reference = result.body().get("reference");
                assertTrue(reference instanceof String);
                assertEquals(uri.toString(), context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE)
                        .getString("keyfile." + reference, null));
                boolean retained = false;
                for (UriPermission permission : resolver.getPersistedUriPermissions()) {
                    if (permission.getUri().equals(uri) && permission.isReadPermission()) retained = true;
                }
                assertTrue(retained);
            }
        }

        void awaitDestructionAndWorkers() throws Exception {
            AtomicBoolean destroyed = new AtomicBoolean();
            long deadline = SystemClock.elapsedRealtime() + 5000;
            do {
                onMain(() -> destroyed.set(activity.isDestroyed()));
                if (destroyed.get()) break;
                SystemClock.sleep(20);
            } while (SystemClock.elapsedRealtime() < deadline);
            assertTrue(destroyed.get());
            for (Field field : HidlinsPlatformServices.class.getDeclaredFields()) {
                field.setAccessible(true);
                if (field.get(services) instanceof ExecutorService worker) {
                    assertTrue(worker.isShutdown());
                    assertTrue(worker.awaitTermination(5, TimeUnit.SECONDS));
                }
            }
            // The worker must have posted any terminal delivery before it
            // terminates. This fence observes that delivery before counting.
            onMain(() -> { });
        }

        void verifyNoNewImport() throws Exception {
            File[] files = vaults().listFiles();
            Set<String> after = new HashSet<>();
            if (files != null) for (File file : files) after.add(file.getCanonicalPath());
            assertEquals(beforeFiles, after);
            if (keyfile) {
                assertEquals(beforeReferences, context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE).getAll());
                Set<String> grants = new HashSet<>();
                for (UriPermission permission : resolver.getPersistedUriPermissions()) {
                    if (permission.isReadPermission()) grants.add(permission.getUri().toString());
                }
                assertEquals(beforeGrants, grants);
            }
        }

        @Override public void close() {
            try {
                cleanup();
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError("fixture cleanup interrupted", error);
            } catch (Exception error) {
                throw new AssertionError("fixture cleanup failed", error);
            }
        }

        private void cleanup() throws Exception {
            release();
            if (dispatch != null) dispatch.await();
            onMain(activity::finish);
            awaitDestructionAndWorkers();
            if (result.replied.await(5, TimeUnit.SECONDS) && result.value.get() instanceof Map<?, ?> map
                    && map.get("value") instanceof Map<?, ?> body) {
                if (body.get("sourceReference") instanceof String path) {
                    File imported = new File(path);
                    if (imported.getParentFile().getCanonicalPath().equals(vaults().getCanonicalPath())
                            && !beforeFiles.contains(imported.getCanonicalPath())) {
                        assertTrue(!imported.exists() || imported.delete());
                    }
                }
                if (body.get("reference") instanceof String reference) {
                    assertTrue(context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE)
                            .edit().remove("keyfile." + reference).commit());
                }
            }
            if (keyfile) {
                try { resolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION); }
                catch (SecurityException ignored) { }
            }
            // Remove only files created after this isolated fixture's snapshot,
            // after all owned provider work has quiesced, including failed tests.
            File[] files = vaults().listFiles();
            if (files != null) for (File file : files) {
                if (!beforeFiles.contains(file.getCanonicalPath())) assertTrue(!file.exists() || file.delete());
            }
            if (keyfile) {
                android.content.SharedPreferences preferences = context.getSharedPreferences("hidlins-keyfiles", Context.MODE_PRIVATE);
                for (Map.Entry<String, ?> entry : preferences.getAll().entrySet()) {
                    if (entry.getKey().startsWith("keyfile.") && uri.toString().equals(entry.getValue())
                            && !beforeReferences.containsKey(entry.getKey())) assertTrue(preferences.edit().remove(entry.getKey()).commit());
                }
            }
            resolver.call(PROVIDER, "remove", id, null);
        }
    }

    static void vaultStageDoesNotBlockMain(Instrumentation instrumentation, String phase) throws Exception {
        responsive(instrumentation, phase, false);
    }

    static void keyfileQueryDoesNotBlockMain(Instrumentation instrumentation) throws Exception {
        responsive(instrumentation, "query", true);
    }

    static void destroyDuringProviderWork(Instrumentation instrumentation, String phase, boolean keyfile)
            throws Exception {
        try (Fixture fixture = new Fixture(instrumentation, phase, keyfile, phase.equals("open"))) {
            fixture.begin();
            MainTask destroy = new MainTask(fixture.activity::finish);
            boolean completedBeforeRelease;
            boolean destroyedBeforeRelease = false;
            boolean responsiveBeforeRelease = false;
            try {
                completedBeforeRelease = fixture.result.replied.await(1, TimeUnit.SECONDS);
                long deadline = SystemClock.elapsedRealtime() + 1000;
                while (SystemClock.elapsedRealtime() < deadline) {
                    AtomicBoolean destroyed = new AtomicBoolean();
                    MainTask heartbeat = new MainTask(() -> destroyed.set(fixture.activity.isDestroyed()));
                    responsiveBeforeRelease = heartbeat.done.await(150, TimeUnit.MILLISECONDS);
                    if (responsiveBeforeRelease && destroyed.get()) { destroyedBeforeRelease = true; break; }
                    SystemClock.sleep(20);
                }
                assertFalse(fixture.state().getBoolean("released"));
            } finally {
                fixture.release();
                fixture.complete();
                destroy.await();
                fixture.awaitDestructionAndWorkers();
            }
            assertTrue(completedBeforeRelease);
            assertTrue(responsiveBeforeRelease);
            assertTrue(destroyedBeforeRelease);
            assertEquals(1, fixture.result.calls.get());
            assertTrue(fixture.result.onMain.get());
            assertEquals("failure", fixture.result.envelope().get("status"));
            assertEquals("activity-destroyed", fixture.result.envelope().get("code"));
            fixture.verifyNoNewImport();
            onMain(() -> assertFalse(fixture.services.beginPicker(91, new Result())));
            if (phase.equals("open")) {
                long deadline = SystemClock.elapsedRealtime() + 2000;
                while (!fixture.state().getBoolean("readerClosed") && SystemClock.elapsedRealtime() < deadline) SystemClock.sleep(20);
                assertTrue(fixture.state().getBoolean("readerClosed"));
                assertFalse(fixture.state().getBoolean("readerCloseTimedOut"));
            }
        }
    }

    static void disposalAfterInstallBeforeDelivery(Instrumentation instrumentation) throws Exception {
        try (Fixture fixture = new Fixture(instrumentation, "none", false)) {
            CountDownLatch dispose = new CountDownLatch(1);
            onMain(() -> assertTrue(fixture.services.beginPicker(PICK_VAULT, fixture.result)));
            fixture.dispatch = new MainTask(() -> {
                assertTrue(fixture.services.onActivityResult(PICK_VAULT, Activity.RESULT_OK,
                        new Intent().setData(fixture.uri)));
                try {
                    assertTrue(dispose.await(5, TimeUnit.SECONDS));
                } catch (InterruptedException error) {
                    Thread.currentThread().interrupt();
                    throw new AssertionError("fixture delivery gate interrupted", error);
                }
                // Same actual activity-owned controller closed on its main
                // thread before any queued worker success can be accepted.
                fixture.services.close();
                fixture.activity.finish();
            });
            boolean installed = false;
            try {
                long deadline = SystemClock.elapsedRealtime() + 3000;
                while (!installed && SystemClock.elapsedRealtime() < deadline) {
                    File[] files = fixture.vaults().listFiles();
                    if (files != null) for (File file : files) {
                        if (file.getName().endsWith(".kdbx") && !fixture.beforeFiles.contains(file.getCanonicalPath())) installed = true;
                    }
                    if (!installed) SystemClock.sleep(20);
                }
            } finally {
                dispose.countDown();
            }
            fixture.complete();
            fixture.awaitDestructionAndWorkers();
            assertTrue(installed);
            assertEquals(1, fixture.result.calls.get());
            assertTrue(fixture.result.onMain.get());
            assertEquals("failure", fixture.result.envelope().get("status"));
            assertEquals("activity-destroyed", fixture.result.envelope().get("code"));
            fixture.verifyNoNewImport();
        }
    }

    static void deliveredImportSurvivesDestruction(Instrumentation instrumentation) throws Exception {
        try (Fixture fixture = new Fixture(instrumentation, "none", false)) {
            fixture.begin();
            fixture.complete();
            fixture.verifySuccess();
            File imported = new File((String) fixture.result.body().get("sourceReference"));
            onMain(fixture.activity::finish);
            fixture.awaitDestructionAndWorkers();
            assertTrue(imported.isFile());
            assertArrayEquals(HidlinsControlledDocumentProvider.CONTENT, Files.readAllBytes(imported.toPath()));
            assertEquals(1, fixture.result.calls.get());
        }
    }

    private static void responsive(Instrumentation instrumentation, String phase, boolean keyfile) throws Exception {
        try (Fixture fixture = new Fixture(instrumentation, phase, keyfile)) {
            fixture.begin();
            boolean responsive;
            AtomicBoolean acceptedSecond = new AtomicBoolean(true);
            try {
                CountDownLatch heartbeat = new CountDownLatch(1);
                new Handler(Looper.getMainLooper()).post(heartbeat::countDown);
                responsive = heartbeat.await(1, TimeUnit.SECONDS);
                if (responsive) onMain(() -> acceptedSecond.set(fixture.services.beginPicker(71, new Result())));
            } finally {
                // Release before asserting red so the original synchronous
                // implementation cannot strand the main thread or next case.
                fixture.release();
                fixture.complete();
            }
            assertTrue(responsive);
            assertFalse(acceptedSecond.get());
            fixture.verifySuccess();
            Result next = new Result();
            onMain(() -> {
                assertTrue(fixture.services.beginPicker(92, next));
                assertTrue(fixture.services.onActivityResult(92, Activity.RESULT_CANCELED, null));
            });
            assertTrue(next.replied.await(1, TimeUnit.SECONDS));
            assertEquals("canceled", next.envelope().get("status"));
            assertEquals(1, next.calls.get());
            assertTrue(next.onMain.get());
            onMain(fixture.activity::finish);
            fixture.awaitDestructionAndWorkers();
            assertEquals(1, fixture.result.calls.get());
            assertEquals(1, next.calls.get());
        }
    }
}
