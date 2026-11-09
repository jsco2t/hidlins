package app.hidlins;

import static app.hidlins.DeviceAssertions.assertArrayEquals;
import static app.hidlins.DeviceAssertions.assertEquals;
import static app.hidlins.DeviceAssertions.assertFalse;
import static app.hidlins.DeviceAssertions.assertNotNull;
import static app.hidlins.DeviceAssertions.assertTrue;

import android.app.Instrumentation;
import android.content.Context;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/** Resource/atomic-install invariants required before provider work overlaps activities. */
final class SafStorageRegressions {
    private static final class TrackedInput extends ByteArrayInputStream {
        final AtomicBoolean closed = new AtomicBoolean();
        TrackedInput() { super(new byte[]{1, 2, 3, 4}); }
        @Override public void close() throws IOException { closed.set(true); super.close(); }
    }

    private static File root(Instrumentation instrumentation) {
        Context context = instrumentation.getTargetContext();
        return new File(context.getCacheDir(), "saf-storage-" + UUID.randomUUID());
    }

    private static void removeOwned(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) removeOwned(child);
        assertTrue(!file.exists() || file.delete());
    }

    static void closesInputOnSetupFailure(Instrumentation instrumentation) throws Exception {
        for (boolean invalidRoot : new boolean[]{true, false}) {
            File root = root(instrumentation);
            TrackedInput input = new TrackedInput();
            boolean failed = false;
            boolean closedByImport;
            try {
                if (invalidRoot) Files.write(root.toPath(), new byte[]{0});
                try {
                    new AppStorage(instrumentation.getTargetContext(), root)
                            .importVault(input, invalidRoot ? "fixture.kdbx" : "...");
                } catch (IOException expected) {
                    failed = true;
                }
                closedByImport = input.closed.get();
            } finally {
                input.close();
                removeOwned(root);
            }
            assertTrue(failed);
            assertTrue(closedByImport);
        }
    }

    static void cleansPartialReadFailure(Instrumentation instrumentation) throws Exception {
        File root = root(instrumentation);
        AtomicBoolean closed = new AtomicBoolean();
        InputStream input = new InputStream() {
            int reads;
            @Override public int read() throws IOException { throw new IOException("fixture read failure"); }
            @Override public int read(byte[] bytes, int offset, int length) throws IOException {
                if (reads++ != 0) throw new IOException("fixture read failure");
                bytes[offset] = 7;
                return 1;
            }
            @Override public void close() { closed.set(true); }
        };
        boolean failed = false;
        try {
            try {
                new AppStorage(instrumentation.getTargetContext(), root).importVault(input, "fixture.kdbx");
            } catch (IOException expected) {
                failed = true;
            }
            assertTrue(failed);
            assertTrue(closed.get());
            // The staging file lives inside Vaults, not the support root.
            File[] contents = new File(root, "Vaults").listFiles();
            assertNotNull(contents);
            assertEquals(0, contents.length);
        } finally {
            input.close();
            removeOwned(root);
        }
    }

    private static InputStream synchronizedInput(byte marker, CountDownLatch atEnd) {
        return new ByteArrayInputStream(new byte[]{marker}) {
            @Override public synchronized int read(byte[] bytes, int offset, int length) {
                int count = super.read(bytes, offset, length);
                if (count == -1) {
                    atEnd.countDown();
                    try {
                        assertTrue(atEnd.await(5, TimeUnit.SECONDS));
                    } catch (InterruptedException error) {
                        Thread.currentThread().interrupt();
                        throw new AssertionError("fixture stream interrupted", error);
                    }
                }
                return count;
            }
        };
    }

    static void overlappingImportsNeverOverwrite(Instrumentation instrumentation) throws Exception {
        File root = root(instrumentation);
        ExecutorService workers = Executors.newFixedThreadPool(2);
        CountDownLatch atEnd = new CountDownLatch(2);
        AppStorage firstStorage = new AppStorage(instrumentation.getTargetContext(), root);
        AppStorage secondStorage = new AppStorage(instrumentation.getTargetContext(), root);
        try {
            // Establish directories before overlapping copies. Only installation
            // and name selection are under test, not a mkdir scheduling race.
            assertTrue(new File(root, "Vaults").mkdirs());
            Future<File> first = workers.submit(() -> firstStorage.importVault(synchronizedInput((byte) 1, atEnd), "same.kdbx"));
            Future<File> second = workers.submit(() -> secondStorage.importVault(synchronizedInput((byte) 2, atEnd), "same.kdbx"));
            File a = first.get(10, TimeUnit.SECONDS);
            File b = second.get(10, TimeUnit.SECONDS);
            assertFalse(a.getCanonicalPath().equals(b.getCanonicalPath()));
            assertArrayEquals(new byte[]{1}, Files.readAllBytes(a.toPath()));
            assertArrayEquals(new byte[]{2}, Files.readAllBytes(b.toPath()));
        } finally {
            workers.shutdownNow();
            assertTrue(workers.awaitTermination(5, TimeUnit.SECONDS));
            removeOwned(root);
        }
    }
}
